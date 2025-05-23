//! Main Redpanda/Kafka consumer implementation

use crate::consumer::{
    config::ConsumerConfig,
    error::{ConsumerError, ConsumerResult},
    processor::MessageProcessor,
    metrics::ConsumerMetrics,
    offset_manager::{OffsetManager, CommitStrategy},
    backpressure::BackpressureController,
    retry::{RetryExecutor, RetryResult},
    dlq::DlqProducer,
    shutdown::ShutdownState,
};

use rdkafka::{
    ClientConfig,
    consumer::{Consumer, StreamConsumer},
    Message,
};
use futures::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Main Redpanda consumer
pub struct RedpandaConsumer<P: MessageProcessor> {
    config: ConsumerConfig,
    processor: Arc<P>,
    consumer: Arc<StreamConsumer>,
    offset_manager: Arc<OffsetManager>,
    metrics: Arc<ConsumerMetrics>,
    backpressure: Arc<BackpressureController>,
    _commit_strategy: CommitStrategy,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    dlq_producer: Option<Arc<DlqProducer>>,
    shutdown_state: Arc<ShutdownState>,
}

impl<P: MessageProcessor> RedpandaConsumer<P> {
    /// Create a new consumer
    pub async fn new(config: ConsumerConfig, processor: P) -> ConsumerResult<Self> {
        // Validate configuration
        config.validate().map_err(ConsumerError::ConfigError)?;
        
        // Define allowed Kafka properties for security
        const ALLOWED_KAFKA_PROPS: &[&str] = &[
            // Compression settings
            "compression.type",
            "compression.level",
            // Fetch settings
            "fetch.min.bytes",
            "fetch.max.wait.ms",
            "fetch.max.bytes",
            "max.partition.fetch.bytes",
            // Request settings
            "request.timeout.ms",
            "metadata.max.age.ms",
            "receive.buffer.bytes",
            "send.buffer.bytes",
            // Consumer settings
            "queued.min.messages",
            "queued.max.messages.kbytes",
            "fetch.error.backoff.ms",
            "fetch.message.max.bytes",
            // Performance settings
            "enable.idempotence",
            "message.max.bytes",
            // Connection settings
            "reconnect.backoff.ms",
            "reconnect.backoff.max.ms",
            "connections.max.idle.ms",
            "socket.keepalive.enable",
            // Monitoring
            "statistics.interval.ms",
            "enable.metrics.push",
        ];
        
        // Create Kafka consumer
        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", &config.brokers)
            .set("group.id", &config.group_id)
            .set("enable.auto.commit", config.enable_auto_commit.to_string())
            .set("auto.commit.interval.ms", config.auto_commit_interval_ms.to_string())
            .set("session.timeout.ms", config.session_timeout_ms.to_string())
            .set("max.poll.interval.ms", config.max_poll_interval_ms.to_string())
            .set("auto.offset.reset", &config.auto_offset_reset);
        
        // Add custom properties with validation
        for (key, value) in &config.kafka_properties {
            if !ALLOWED_KAFKA_PROPS.contains(&key.as_str()) {
                return Err(ConsumerError::ConfigError(
                    format!("Disallowed Kafka property '{}'. Allowed properties: {:?}", 
                            key, ALLOWED_KAFKA_PROPS)
                ));
            }
            client_config.set(key, value);
        }
        
        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| ConsumerError::ConnectionError(format!("Failed to create consumer: {}", e)))?;
        
        // Subscribe to topics with timeout
        let topics: Vec<&str> = config.topics.iter().map(|s| s.as_str()).collect();
        tokio::time::timeout(
            Duration::from_secs(30), // subscription timeout
            async {
                consumer.subscribe(&topics)
                    .map_err(|e| ConsumerError::ConnectionError(format!("Failed to subscribe: {}", e)))
            }
        ).await
        .map_err(|_| ConsumerError::ConnectionError("Subscription timeout".to_string()))??;
        
        info!("Subscribed to topics: {:?}", config.topics);
        
        // Create DLQ producer if configured
        let dlq_producer = if let Some(dlq_topic) = &config.dlq_topic {
            let mut dlq_config = ClientConfig::new();
            dlq_config
                .set("bootstrap.servers", &config.brokers)
                .set("message.timeout.ms", "30000");
            
            let producer: rdkafka::producer::FutureProducer = dlq_config
                .create()
                .map_err(|e| ConsumerError::ConnectionError(format!("Failed to create DLQ producer: {}", e)))?;
            
            let dlq = DlqProducer::new(producer, dlq_topic.clone())
                .with_timeout(Duration::from_secs(30))
                .with_metadata(true);
            
            info!("Created DLQ producer for topic: {}", dlq_topic);
            Some(Arc::new(dlq))
        } else {
            None
        };
        
        // Create components
        let offset_manager = Arc::new(OffsetManager::new(
            config.batch_size,
            config.metrics_interval,
        ));
        
        let metrics = Arc::new(ConsumerMetrics::new());
        
        let backpressure = Arc::new(BackpressureController::new(
            config.max_inflight_messages,
            config.pause_threshold,
            config.resume_threshold,
        ));
        
        let commit_strategy = CommitStrategy::BatchOrInterval(
            config.batch_size,
            Duration::from_secs(5),
        );
        
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let shutdown_state = Arc::new(ShutdownState::new());
        
        Ok(Self {
            config,
            processor: Arc::new(processor),
            consumer: Arc::new(consumer),
            offset_manager,
            metrics,
            backpressure,
            _commit_strategy: commit_strategy,
            shutdown_tx,
            shutdown_rx,
            dlq_producer,
            shutdown_state,
        })
    }
    
    /// Run the consumer
    pub async fn run(self) -> ConsumerResult<()> {
        info!("Starting Redpanda consumer with {} workers", self.config.num_workers);
        
        // Create processing channel
        let (task_tx, main_task_rx) = mpsc::channel::<ProcessingTask>(self.config.channel_buffer_size);
        
        // Start background tasks
        let mut handles = vec![];
        
        // Start consumer loop
        let consumer_handle = self.spawn_consumer_loop(task_tx.clone());
        handles.push(consumer_handle);
        
        // Start worker distributor and processor workers
        let worker_handles = self.spawn_worker_pool(main_task_rx).await;
        handles.extend(worker_handles);
        
        // Start metrics reporter
        let metrics_handle = self.spawn_metrics_reporter();
        handles.push(metrics_handle);
        
        // Start offset committer
        let commit_handle = self.spawn_offset_committer();
        handles.push(commit_handle);
        
        // Wait for shutdown signal
        let mut shutdown_rx = self.shutdown_rx.clone();
        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("Shutdown signal received");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl-C received, shutting down");
                self.shutdown_tx.send(true).ok();
            }
        }
        
        // Graceful shutdown
        info!("Starting graceful shutdown");
        
        // Close channel to stop accepting new messages
        // The consumer loop will stop when shutdown signal is sent
        
        // Adaptive wait for inflight messages with timeout
        let shutdown_deadline = Instant::now() + Duration::from_secs(30);
        let mut last_inflight_count = None;
        
        while self.shutdown_state.has_inflight_messages().await && Instant::now() < shutdown_deadline {
            let current_inflight = self.shutdown_state.inflight_count().await;
            
            // Log progress periodically
            if last_inflight_count != Some(current_inflight) {
                info!("Waiting for {} inflight messages to complete", current_inflight);
                last_inflight_count = Some(current_inflight);
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        if self.shutdown_state.has_inflight_messages().await {
            warn!("Shutdown timeout reached with {} messages still in flight", 
                  self.shutdown_state.inflight_count().await);
        }
        
        // Commit final offsets with timeout
        match tokio::time::timeout(
            Duration::from_secs(10),
            self.offset_manager.commit_offsets(&*self.consumer)
        ).await {
            Ok(Ok(())) => info!("Final offsets committed successfully"),
            Ok(Err(e)) => error!("Failed to commit final offsets: {}", e),
            Err(_) => error!("Timeout committing final offsets"),
        }
        
        // Graceful shutdown of background tasks with timeout
        let handles_count = handles.len();
        info!("Initiating graceful shutdown of {} background tasks", handles_count);
        
        let shutdown_timeout = Duration::from_secs(30);
        let mut successful_shutdowns = 0;
        
        for (i, handle) in handles.into_iter().enumerate() {
            match tokio::time::timeout(shutdown_timeout, handle).await {
                Ok(Ok(())) => {
                    successful_shutdowns += 1;
                    debug!("Task {} shutdown gracefully", i);
                }
                Ok(Err(e)) => {
                    warn!("Task {} completed with error during shutdown: {}", i, e);
                    successful_shutdowns += 1;
                }
                Err(_) => {
                    warn!("Task {} did not shutdown within timeout, forcing termination", i);
                    // Note: JoinHandle is already dropped here, so task is aborted
                }
            }
        }
        
        info!("Shutdown complete: {}/{} tasks shutdown gracefully", 
              successful_shutdowns, handles_count);
        
        info!("Consumer shutdown complete");
        Ok(())
    }
    
    /// Spawn the main consumer loop
    fn spawn_consumer_loop(&self, task_tx: mpsc::Sender<ProcessingTask>) -> JoinHandle<()> {
        let consumer = self.consumer.clone();
        let metrics = self.metrics.clone();
        let backpressure = self.backpressure.clone();
        let mut shutdown_rx = self.shutdown_rx.clone();
        
        tokio::spawn(async move {
            let mut stream = consumer.stream();
            
            while !*shutdown_rx.borrow() {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        info!("Consumer loop received shutdown signal");
                        break;
                    }
                    message = stream.next() => {
                        match message {
                            Some(Ok(msg)) => {
                                metrics.increment_consumed();
                                
                                // Check backpressure
                                if backpressure.should_pause() {
                                    if let Ok(assignment) = consumer.assignment() {
                                        consumer.pause(&assignment).ok();
                                    }
                                    continue;
                                }
                                
                                if backpressure.should_resume() {
                                    if let Ok(assignment) = consumer.assignment() {
                                        consumer.resume(&assignment).ok();
                                    }
                                }
                                
                                // Create processing task
                                let task = ProcessingTask {
                                    message: msg.detach(),
                                    attempt: 0,
                                    start_time: Instant::now(),
                                };
                                
                                // Send to processing queue
                                if task_tx.send(task).await.is_err() {
                                    warn!("Processing channel closed");
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                error!("Kafka error: {}", e);
                                metrics.record_error("kafka_error");
                            }
                            None => break,
                        }
                    }
                }
            }
        })
    }
    
    /// Spawn worker pool with distributor
    async fn spawn_worker_pool(&self, main_task_rx: mpsc::Receiver<ProcessingTask>) -> Vec<JoinHandle<()>> {
        let mut handles = vec![];
        
        if self.config.num_workers == 1 {
            // Single worker - direct connection for efficiency
            let worker_handle = self.spawn_processor_worker(0, main_task_rx);
            handles.push(worker_handle);
        } else {
            // Multiple workers - use distributor pattern
            let worker_channels: Vec<_> = (0..self.config.num_workers)
                .map(|_| mpsc::channel::<ProcessingTask>(self.config.channel_buffer_size / self.config.num_workers))
                .collect();
            
            // Spawn distributor task
            let distributor_handle = self.spawn_distributor_task(main_task_rx, worker_channels.iter().map(|(tx, _)| tx.clone()).collect());
            handles.push(distributor_handle);
            
            // Spawn worker tasks
            for (worker_id, (_, rx)) in worker_channels.into_iter().enumerate() {
                let worker_handle = self.spawn_processor_worker(worker_id, rx);
                handles.push(worker_handle);
            }
        }
        
        handles
    }
    
    /// Spawn distributor task that round-robin distributes work
    fn spawn_distributor_task(
        &self,
        mut main_rx: mpsc::Receiver<ProcessingTask>,
        worker_senders: Vec<mpsc::Sender<ProcessingTask>>,
    ) -> JoinHandle<()> {
        let shutdown_state = self.shutdown_state.clone();
        
        // Worker health tracking with circuit breaker pattern
        #[derive(Debug)]
        struct WorkerState {
            failures: u32,
            last_failure: Option<Instant>,
            circuit_open: bool,
        }
        
        tokio::spawn(async move {
            info!("Distributor task started with {} workers", worker_senders.len());
            let mut current_worker = 0;
            
            // Initialize worker states
            let mut worker_states: Vec<WorkerState> = (0..worker_senders.len())
                .map(|_| WorkerState {
                    failures: 0,
                    last_failure: None,
                    circuit_open: false,
                })
                .collect();
            
            const MAX_FAILURES: u32 = 5;
            const CIRCUIT_RESET_DURATION: Duration = Duration::from_secs(30);
            
            while let Some(mut task) = main_rx.recv().await {
                // Try to distribute to workers, starting with current worker
                let mut sent = false;
                let mut attempts = 0;
                
                for _ in 0..worker_senders.len() {
                    let worker_idx = (current_worker + attempts) % worker_senders.len();
                    let worker_state = &mut worker_states[worker_idx];
                    
                    // Check if circuit should be reset
                    if worker_state.circuit_open {
                        if let Some(last_failure) = worker_state.last_failure {
                            if last_failure.elapsed() > CIRCUIT_RESET_DURATION {
                                info!("Resetting circuit for worker {}", worker_idx);
                                worker_state.circuit_open = false;
                                worker_state.failures = 0;
                            }
                        }
                    }
                    
                    // Skip if circuit is open
                    if worker_state.circuit_open {
                        attempts += 1;
                        continue;
                    }
                    
                    match worker_senders[worker_idx].send(task).await {
                        Ok(()) => {
                            sent = true;
                            // Reset failure count on success
                            worker_state.failures = 0;
                            break;
                        }
                        Err(send_error) => {
                            task = send_error.0; // Extract the task from the SendError
                            worker_state.failures += 1;
                            worker_state.last_failure = Some(Instant::now());
                            
                            if worker_state.failures >= MAX_FAILURES {
                                warn!("Worker {} circuit breaker opened after {} failures", 
                                      worker_idx, MAX_FAILURES);
                                worker_state.circuit_open = true;
                            }
                            
                            attempts += 1;
                        }
                    }
                }
                
                if !sent {
                    error!("Failed to distribute task to any worker - all circuits may be open");
                    // Ensure inflight message is removed to prevent deadlock
                    shutdown_state.remove_inflight_message().await;
                    
                    // Check if all workers are failed
                    let all_failed = worker_states.iter().all(|s| s.circuit_open);
                    if all_failed {
                        error!("All worker circuits are open, exiting distributor");
                        break;
                    }
                }
                
                // Move to next worker
                current_worker = (current_worker + 1) % worker_senders.len();
            }
            
            info!("Distributor task finished");
        })
    }
    
    /// Spawn a processor worker
    fn spawn_processor_worker(
        &self,
        worker_id: usize,
        task_rx: mpsc::Receiver<ProcessingTask>,
    ) -> JoinHandle<()> {
        let processor = self.processor.clone();
        let metrics = self.metrics.clone();
        let offset_manager = self.offset_manager.clone();
        let backpressure = self.backpressure.clone();
        let config = self.config.clone();
        let dlq_producer = self.dlq_producer.clone();
        let shutdown_state = self.shutdown_state.clone();
        
        tokio::spawn(async move {
            info!("Processor worker {} started", worker_id);
            
            if config.enable_batching {
                // Batch processing mode
                Self::run_batch_processor(
                    worker_id,
                    task_rx,
                    processor,
                    metrics,
                    offset_manager,
                    backpressure,
                    config,
                    dlq_producer,
                    shutdown_state,
                ).await;
            } else {
                // Single message processing mode
                Self::run_single_processor(
                    worker_id,
                    task_rx,
                    processor,
                    metrics,
                    offset_manager,
                    backpressure,
                    config,
                    dlq_producer,
                    shutdown_state,
                ).await;
            }
        })
    }
    
    /// Run batch processor
    async fn run_batch_processor(
        worker_id: usize,
        mut task_rx: mpsc::Receiver<ProcessingTask>,
        processor: Arc<P>,
        metrics: Arc<ConsumerMetrics>,
        offset_manager: Arc<OffsetManager>,
        backpressure: Arc<BackpressureController>,
        config: ConsumerConfig,
        dlq_producer: Option<Arc<crate::consumer::dlq::DlqProducer>>,
        shutdown_state: Arc<crate::consumer::shutdown::ShutdownState>,
    ) {
        let mut batch = Vec::with_capacity(config.batch_size);
        let mut batch_timer = tokio::time::interval(config.batch_timeout);
        batch_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        loop {
            tokio::select! {
                // Receive new task
                task_option = task_rx.recv() => {
                    match task_option {
                        Some(task) => {
                            batch.push(task);
                            
                            // Process batch if it's full
                            if batch.len() >= config.batch_size {
                                Self::process_batch(
                                    &batch,
                                    &processor,
                                    &metrics,
                                    &offset_manager,
                                    &backpressure,
                                    &config,
                                    &dlq_producer,
                                    &shutdown_state,
                                ).await;
                                batch.clear();
                            }
                        }
                        None => {
                            // Channel closed, process remaining batch and exit
                            if !batch.is_empty() {
                                Self::process_batch(
                                    &batch,
                                    &processor,
                                    &metrics,
                                    &offset_manager,
                                    &backpressure,
                                    &config,
                                    &dlq_producer,
                                    &shutdown_state,
                                ).await;
                            }
                            break;
                        }
                    }
                }
                
                // Batch timeout - process current batch
                _ = batch_timer.tick() => {
                    if !batch.is_empty() {
                        Self::process_batch(
                            &batch,
                            &processor,
                            &metrics,
                            &offset_manager,
                            &backpressure,
                            &config,
                            &dlq_producer,
                            &shutdown_state,
                        ).await;
                        batch.clear();
                    }
                }
            }
        }
        
        info!("Batch processor worker {} finished", worker_id);
    }
    
    /// Run single message processor
    async fn run_single_processor(
        worker_id: usize,
        mut task_rx: mpsc::Receiver<ProcessingTask>,
        processor: Arc<P>,
        metrics: Arc<ConsumerMetrics>,
        offset_manager: Arc<OffsetManager>,
        backpressure: Arc<BackpressureController>,
        config: ConsumerConfig,
        dlq_producer: Option<Arc<crate::consumer::dlq::DlqProducer>>,
        shutdown_state: Arc<crate::consumer::shutdown::ShutdownState>,
    ) {
        while let Some(mut task) = task_rx.recv().await {
            // Track inflight message
            shutdown_state.add_inflight_message().await;
                
            // Acquire backpressure permit
            let _permit = match backpressure.acquire().await {
                Ok(permit) => permit,
                Err(e) => {
                    error!("Failed to acquire backpressure permit: {}", e);
                    metrics.record_error("backpressure_error");
                    shutdown_state.remove_inflight_message().await;
                    continue;
                }
            };
            
            // Process message with retry
            let result = process_message_with_retry(
                processor.as_ref(),
                &task.message,
                &config,
                &mut task.attempt,
            ).await;
            
            let processing_duration = task.start_time.elapsed();
            
            match result {
                Ok(()) => {
                    metrics.increment_processed();
                    processor.on_success(&task.message).await;
                    
                    // Mark offset for commit
                    offset_manager.mark_offset(
                        task.message.topic(),
                        task.message.partition(),
                        task.message.offset(),
                    ).await;
                    
                    // Record success in backpressure controller
                    backpressure.record_success(processing_duration).await;
                }
                Err(e) => {
                    metrics.increment_failed();
                    processor.on_failure(&e, &task.message).await;
                    
                    // Send to DLQ if configured and max retries exceeded
                    if task.attempt >= config.dlq_after_retries {
                        if let Some(dlq) = &dlq_producer {
                            if let Err(dlq_err) = dlq.send_message(&task.message, &e.to_string(), task.attempt).await {
                                error!("Failed to send to DLQ: {}", dlq_err);
                                metrics.increment_dlq_failures();
                                metrics.record_error("dlq_send_failed");
                            } else {
                                metrics.increment_dlq();
                            }
                        }
                    }
                    
                    // Record failure in backpressure controller
                    backpressure.record_failure().await;
                }
            }
            
            // Record processing duration
            metrics.record_processing_duration(processing_duration);
            
            // Update message size estimate (if we have payload size)
            if let Some(payload) = task.message.payload() {
                backpressure.update_avg_message_size(payload.len());
            }
            
            // Remove inflight message
            shutdown_state.remove_inflight_message().await;
        }
        
        info!("Single processor worker {} finished", worker_id);
    }
    
    /// Process a batch of messages
    async fn process_batch(
        batch: &[ProcessingTask],
        processor: &Arc<P>,
        metrics: &Arc<ConsumerMetrics>,
        offset_manager: &Arc<OffsetManager>,
        backpressure: &Arc<BackpressureController>,
        config: &ConsumerConfig,
        dlq_producer: &Option<Arc<crate::consumer::dlq::DlqProducer>>,
        shutdown_state: &Arc<crate::consumer::shutdown::ShutdownState>,
    ) {
        if batch.is_empty() {
            return;
        }
        
        debug!("Processing batch of {} messages", batch.len());
        let batch_start = std::time::Instant::now();
        
        // Track inflight messages
        for _ in batch {
            shutdown_state.add_inflight_message().await;
        }
        
        // Acquire backpressure permits for the entire batch
        let mut permits = Vec::with_capacity(batch.len());
        for i in 0..batch.len() {
            match backpressure.acquire().await {
                Ok(permit) => permits.push(permit),
                Err(e) => {
                    error!("Failed to acquire backpressure permit for batch item {}: {}", i, e);
                    metrics.record_error("backpressure_error");
                    // Remove the inflight messages we added for items we couldn't get permits for
                    for _ in i..batch.len() {
                        shutdown_state.remove_inflight_message().await;
                    }
                    // Process only the items we got permits for
                    if i == 0 {
                        return; // No permits acquired, can't process any items
                    }
                    break;
                }
            }
        }
        
        // Process each message in the batch
        for task in batch {
            let mut attempt = task.attempt;
            let result = process_message_with_retry(
                processor.as_ref(),
                &task.message,
                config,
                &mut attempt,
            ).await;
            
            let processing_duration = task.start_time.elapsed();
            
            match result {
                Ok(()) => {
                    metrics.increment_processed();
                    processor.on_success(&task.message).await;
                    
                    // Mark offset for commit
                    offset_manager.mark_offset(
                        task.message.topic(),
                        task.message.partition(),
                        task.message.offset(),
                    ).await;
                    
                    // Record success in backpressure controller
                    backpressure.record_success(processing_duration).await;
                }
                Err(e) => {
                    metrics.increment_failed();
                    processor.on_failure(&e, &task.message).await;
                    
                    // Send to DLQ if configured and max retries exceeded
                    if attempt >= config.dlq_after_retries {
                        if let Some(dlq) = dlq_producer {
                            if let Err(dlq_err) = dlq.send_message(&task.message, &e.to_string(), attempt).await {
                                error!("Failed to send to DLQ: {}", dlq_err);
                                metrics.increment_dlq_failures();
                                metrics.record_error("dlq_send_failed");
                            } else {
                                metrics.increment_dlq();
                            }
                        }
                    }
                    
                    // Record failure in backpressure controller
                    backpressure.record_failure().await;
                }
            }
            
            // Record processing duration
            metrics.record_processing_duration(processing_duration);
            
            // Update message size estimate (if we have payload size)
            if let Some(payload) = task.message.payload() {
                backpressure.update_avg_message_size(payload.len());
            }
            
            // Remove inflight message
            shutdown_state.remove_inflight_message().await;
        }
        
        debug!("Batch of {} messages processed in {:?}", batch.len(), batch_start.elapsed());
    }
    
    /// Spawn metrics reporter
    fn spawn_metrics_reporter(&self) -> JoinHandle<()> {
        let metrics = self.metrics.clone();
        let interval = self.config.metrics_interval;
        let mut shutdown_rx = self.shutdown_rx.clone();
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let stats = metrics.processing_stats();
                        info!(
                            "Consumer stats - Messages/sec: {:.2}, Success rate: {:.2}%, P99 latency: {:?}",
                            metrics.messages_per_second(),
                            metrics.success_rate() * 100.0,
                            stats.p99
                        );
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        })
    }
    
    /// Spawn offset committer
    fn spawn_offset_committer(&self) -> JoinHandle<()> {
        let consumer = self.consumer.clone();
        let offset_manager = self.offset_manager.clone();
        let metrics = self.metrics.clone();
        let mut shutdown_rx = self.shutdown_rx.clone();
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let start = Instant::now();
                        if let Err(e) = offset_manager.commit_offsets(&*consumer).await {
                            error!("Failed to commit offsets: {}", e);
                            metrics.record_error("commit_error");
                        } else {
                            metrics.record_commit_duration(start.elapsed());
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        })
    }
    
    /// Enable adaptive backpressure control
    pub fn spawn_adaptive_controller(&self, config: crate::consumer::backpressure::AdaptiveBackpressureConfig) -> JoinHandle<()> {
        let controller = crate::consumer::backpressure::AdaptiveBackpressureController::new(
            config.initial_inflight,
            config.min_inflight,
            config.max_inflight,
            config.pause_threshold,
            config.resume_threshold,
            config.adjustment_interval,
            config.target_latency,
            config.target_success_rate,
        );
        
        tokio::spawn(async move {
            controller.run_adjustment_loop().await;
        })
    }
    
    /// Shutdown the consumer gracefully
    pub async fn shutdown(&self) -> ConsumerResult<()> {
        info!("Initiating consumer shutdown");
        
        // Start shutdown process
        self.shutdown_state.start_shutdown().await;
        
        // Signal all workers to stop
        self.shutdown_tx.send(true).map_err(|_| ConsumerError::ShutdownError("Failed to send shutdown signal".to_string()))?;
        
        // Wait for all inflight messages to complete
        let timeout = Duration::from_secs(30);
        match self.shutdown_state.wait_for_completion(timeout).await {
            Ok(()) => {
                info!("Graceful shutdown completed");
                Ok(())
            }
            Err(e) => {
                error!("Shutdown timeout: {}", e);
                Err(ConsumerError::ShutdownError(e.to_string()))
            }
        }
    }
}

/// Task for processing a message
struct ProcessingTask {
    message: rdkafka::message::OwnedMessage,
    attempt: u32,
    start_time: Instant,
}

/// Process a message with retry logic
async fn process_message_with_retry<P: MessageProcessor>(
    processor: &P,
    message: &rdkafka::message::OwnedMessage,
    config: &ConsumerConfig,
    attempt: &mut u32,
) -> Result<(), P::Error> {
    let executor = RetryExecutor::new(config.retry_policy.clone());
    
    match executor.execute_with_predicate(
        || async { processor.process(message).await },
        |e| processor.is_retryable(e)
    ).await {
        RetryResult::Success { value, attempts } => {
            *attempt = attempts;
            Ok(value)
        }
        RetryResult::Failed { error, attempts } => {
            *attempt = attempts;
            Err(error)
        }
    }
}
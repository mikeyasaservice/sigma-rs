//! Main Redpanda/Kafka consumer implementation

use crate::consumer::{
    config::ConsumerConfig,
    error::{ConsumerError, ConsumerResult},
    processor::MessageProcessor,
    metrics::ConsumerMetrics,
    offset_manager::{OffsetManager, CommitStrategy},
    backpressure::BackpressureController,
    retry::{RetryPolicy, RetryExecutor, RetryResult},
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
    commit_strategy: CommitStrategy,
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
        
        // Add custom properties
        for (key, value) in &config.kafka_properties {
            client_config.set(key, value);
        }
        
        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| ConsumerError::ConnectionError(format!("Failed to create consumer: {}", e)))?;
        
        // Subscribe to topics
        let topics: Vec<&str> = config.topics.iter().map(|s| s.as_str()).collect();
        consumer
            .subscribe(&topics)
            .map_err(|e| ConsumerError::ConnectionError(format!("Failed to subscribe: {}", e)))?;
        
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
            commit_strategy,
            shutdown_tx,
            shutdown_rx,
            dlq_producer,
            shutdown_state,
        })
    }
    
    /// Run the consumer
    pub async fn run(self) -> ConsumerResult<()> {
        info!("Starting Redpanda consumer");
        
        // Create processing channel
        let (task_tx, mut task_rx) = mpsc::channel::<ProcessingTask>(self.config.channel_buffer_size);
        
        // Start background tasks
        let mut handles = vec![];
        
        // Start consumer loop
        let consumer_handle = self.spawn_consumer_loop(task_tx.clone());
        handles.push(consumer_handle);
        
        // Start processor workers - only one worker for now with mpsc
        // TODO: Use broadcast channel for multiple workers
        let processor_handle = self.spawn_processor_worker(0, task_rx);
        handles.push(processor_handle);
        
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
        
        // Allow time for workers to finish processing
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Commit final offsets
        if let Err(e) = self.offset_manager.commit_offsets(&*self.consumer).await {
            error!("Failed to commit final offsets: {}", e);
        }
        
        // Cancel all background tasks
        for handle in handles {
            handle.abort();
        }
        
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
    
    /// Spawn a processor worker
    fn spawn_processor_worker(
        &self,
        worker_id: usize,
        mut task_rx: mpsc::Receiver<ProcessingTask>,
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
            
            while let Some(mut task) = task_rx.recv().await {
                // Track inflight message
                shutdown_state.add_inflight_message().await;
                
                // Acquire backpressure permit
                let _permit = backpressure.acquire().await;
                
                // Process message with retry
                let result = process_message_with_retry(
                    processor.as_ref(),
                    &task.message,
                    &config,
                    &mut task.attempt,
                ).await;
                
                match result {
                    Ok(()) => {
                        metrics.increment_processed();
                        processor.on_success(&task.message).await;
                        
                        // Mark offset for commit
                        offset_manager.mark_offset(
                            task.message.topic().to_string(),
                            task.message.partition(),
                            task.message.offset(),
                        ).await;
                    }
                    Err(e) => {
                        metrics.increment_failed();
                        processor.on_failure(&e, &task.message).await;
                        
                        // Send to DLQ if configured and max retries exceeded
                        if task.attempt >= config.dlq_after_retries {
                            if let Some(dlq) = &dlq_producer {
                                if let Err(e) = dlq.send_message(&task.message, &e.to_string(), task.attempt).await {
                                    error!("Failed to send to DLQ: {}", e);
                                } else {
                                    metrics.increment_dlq();
                                }
                            }
                        }
                    }
                }
                
                // Record processing duration
                metrics.record_processing_duration(task.start_time.elapsed());
                
                // Remove inflight message
                shutdown_state.remove_inflight_message().await;
            }
            
            info!("Processor worker {} stopped", worker_id);
        })
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
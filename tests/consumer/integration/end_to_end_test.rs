#[cfg(test)]
mod tests {
    use sigma_rs::consumer::{
        config::{ConsumerConfig, ConsumerConfigBuilder},
        consumer::RedpandaConsumer,
        processor::MessageProcessor,
        error::{ConsumerError, ConsumerResult},
        retry::RetryPolicy,
    };
    use async_trait::async_trait;
    use rdkafka::{
        message::{OwnedMessage, Message},
        producer::{FutureProducer, FutureRecord},
        ClientConfig,
    };
    use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
    use std::time::Duration;
    use testcontainers::{clients, images::kafka};
    use tokio::sync::watch;
    use tokio::time::timeout;
    use tracing::{info, error};
    
    // Test processor that counts messages
    #[derive(Clone)]
    struct CountingProcessor {
        success_count: Arc<AtomicU32>,
        failure_count: Arc<AtomicU32>,
        simulate_failure_rate: f32,
    }
    
    #[async_trait]
    impl MessageProcessor for CountingProcessor {
        type Error = ConsumerError;
        
        async fn process(&self, message: &OwnedMessage) -> Result<(), Self::Error> {
            // Simulate processing
            tokio::time::sleep(Duration::from_millis(10)).await;
            
            // Simulate failures based on rate
            if rand::random::<f32>() < self.simulate_failure_rate {
                self.failure_count.fetch_add(1, Ordering::Relaxed);
                Err(ConsumerError::ProcessingError("Simulated failure".to_string()))
            } else {
                self.success_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        }
        
        async fn on_success(&self, _message: &OwnedMessage) {
            info!("Message processed successfully");
        }
        
        async fn on_failure(&self, error: &Self::Error, _message: &OwnedMessage) {
            error!("Message processing failed: {}", error);
        }
        
        fn is_retryable(&self, _error: &Self::Error) -> bool {
            true
        }
    }
    
    // Helper to create test messages
    async fn produce_test_messages(brokers: &str, topic: &str, count: u32) -> ConsumerResult<()> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .create()
            .map_err(|e| ConsumerError::ConnectionError(format!("Failed to create producer: {}", e)))?;
        
        for i in 0..count {
            let record = FutureRecord::to(topic)
                .key(&format!("key-{}", i))
                .payload(&format!("test-message-{}", i));
            
            producer.send(record, Duration::from_secs(5)).await
                .map_err(|(e, _)| ConsumerError::Generic(format!("Failed to produce: {}", e)))?;
        }
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_full_message_flow() {
        // Start Kafka container
        let docker = clients::Cli::default();
        let kafka_node = docker.run(kafka::Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Wait for Kafka to be ready
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Create processor
        let success_count = Arc::new(AtomicU32::new(0));
        let failure_count = Arc::new(AtomicU32::new(0));
        let processor = CountingProcessor {
            success_count: success_count.clone(),
            failure_count: failure_count.clone(),
            simulate_failure_rate: 0.1, // 10% failure rate
        };
        
        // Create consumer config
        let config = ConsumerConfigBuilder::new()
            .brokers(brokers.clone())
            .group_id("test-group".to_string())
            .topics(vec!["test-topic".to_string()])
            .batch_size(5)
            .max_inflight_messages(10)
            .retry_policy(RetryPolicy {
                max_retries: 2,
                initial_backoff: Duration::from_millis(100),
                max_backoff: Duration::from_secs(1),
                backoff_multiplier: 2.0,
                jitter_factor: 0.1,
                exponential: true,
            })
            .dlq_topic("test-dlq".to_string())
            .build();
        
        // Create consumer
        let consumer = RedpandaConsumer::new(config, processor).await.unwrap();
        
        // Start consumer in background
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let consumer_handle = tokio::spawn(async move {
            consumer.run().await
        });
        
        // Produce test messages
        let message_count = 20;
        produce_test_messages(&brokers, "test-topic", message_count).await.unwrap();
        
        // Let consumer process messages
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Shutdown consumer
        shutdown_tx.send(true).unwrap();
        
        // Wait for consumer to finish with timeout
        let _ = timeout(Duration::from_secs(10), consumer_handle).await;
        
        // Verify results
        let processed = success_count.load(Ordering::Relaxed);
        let failed = failure_count.load(Ordering::Relaxed);
        
        tracing::error!("Processed: {}, Failed: {}", processed, failed);
        
        // With retries and 10% failure rate, we should process most messages
        assert!(processed > 15, "Should process at least 75% of messages");
        assert!(processed + failed >= message_count, "Should attempt all messages");
    }
    
    #[tokio::test]
    async fn test_consumer_group_coordination() {
        // Start Kafka container
        let docker = clients::Cli::default();
        let kafka_node = docker.run(kafka::Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Wait for Kafka to be ready
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Create shared counters
        let success_count = Arc::new(AtomicU32::new(0));
        let failure_count = Arc::new(AtomicU32::new(0));
        
        // Create multiple consumers in same group
        let mut handles = vec![];
        
        for i in 0..3 {
            let success = success_count.clone();
            let failure = failure_count.clone();
            let brokers_clone = brokers.clone();
            
            let handle = tokio::spawn(async move {
                let processor = CountingProcessor {
                    success_count: success,
                    failure_count: failure,
                    simulate_failure_rate: 0.0, // No failures for this test
                };
                
                let config = ConsumerConfigBuilder::new()
                    .brokers(brokers_clone)
                    .group_id("coordinated-group".to_string())
                    .topics(vec!["partitioned-topic".to_string()])
                    .build();
                
                let consumer = RedpandaConsumer::new(config, processor).await.unwrap();
                consumer.run().await
            });
            
            handles.push(handle);
        }
        
        // Produce messages
        produce_test_messages(&brokers, "partitioned-topic", 30).await.unwrap();
        
        // Let consumers process
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Shutdown all consumers
        for handle in handles {
            handle.abort();
        }
        
        let total_processed = success_count.load(Ordering::Relaxed);
        assert_eq!(total_processed, 30, "All messages should be processed exactly once");
    }
    
    #[tokio::test]
    async fn test_performance_under_load() {
        // Start Kafka container
        let docker = clients::Cli::default();
        let kafka_node = docker.run(kafka::Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Wait for Kafka to be ready
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // High-performance processor
        #[derive(Clone)]
        struct FastProcessor;
        
        #[async_trait]
        impl MessageProcessor for FastProcessor {
            type Error = ConsumerError;
            
            async fn process(&self, _message: &OwnedMessage) -> Result<(), Self::Error> {
                // Minimal processing
                Ok(())
            }
            
            async fn on_success(&self, _message: &OwnedMessage) {}
            async fn on_failure(&self, _error: &Self::Error, _message: &OwnedMessage) {}
        }
        
        let config = ConsumerConfigBuilder::new()
            .brokers(brokers.clone())
            .group_id("perf-test-group".to_string())
            .topics(vec!["perf-topic".to_string()])
            .batch_size(100)
            .max_inflight_messages(500)
            .channel_buffer_size(1000)
            .build();
        
        let consumer = RedpandaConsumer::new(config, FastProcessor).await.unwrap();
        
        // Start consumer
        let consumer_handle = tokio::spawn(async move {
            consumer.run().await
        });
        
        // Produce many messages
        let start = std::time::Instant::now();
        produce_test_messages(&brokers, "perf-topic", 1000).await.unwrap();
        
        // Let consumer process
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Calculate throughput
        let duration = start.elapsed();
        let throughput = 1000.0 / duration.as_secs_f64();
        
        tracing::error!("Throughput: {:.2} messages/second", throughput);
        
        // Cleanup
        consumer_handle.abort();
        
        // Should process at reasonable speed (this depends on system)
        assert!(throughput > 100.0, "Should process at least 100 messages/second");
    }
}
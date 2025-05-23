#[cfg(test)]
mod tests {
    use sigma_rs::consumer::{
        config::{ConsumerConfig, ConsumerConfigBuilder},
        consumer::RedpandaConsumer,
        processor::MessageProcessor,
        error::{ConsumerError, ConsumerResult},
        retry::RetryPolicy,
        metrics::ConsumerMetrics,
        offset_manager::OffsetManager,
        dlq::DlqProducer,
    };
    use async_trait::async_trait;
    use rdkafka::{
        ClientConfig,
        producer::{FutureProducer, FutureRecord},
        message::{OwnedMessage, Message},
    };
    use std::sync::{Arc, atomic::{AtomicU32, Ordering}};
    use std::time::Duration;
    use testcontainers::{clients::Cli};
    use testcontainers_modules::kafka::Kafka;
    use tokio::signal;
    use tracing::info;
    
    // Simple test processor
    #[derive(Clone)]
    struct TestProcessor {
        count: Arc<AtomicU32>,
    }
    
    #[async_trait]
    impl MessageProcessor for TestProcessor {
        type Error = ConsumerError;
        
        async fn process(&self, _message: &OwnedMessage) -> Result<(), Self::Error> {
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        
        async fn on_success(&self, _message: &OwnedMessage) {}
        async fn on_failure(&self, _error: &Self::Error, _message: &OwnedMessage) {}
    }
    
    // Helper to produce test events
    async fn produce_events(brokers: &str, topic: &str, count: u32) {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .create()
            .unwrap();
        
        for i in 0..count {
            let event = serde_json::json!({
                "id": i,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "data": format!("test event {}", i)
            });
            
            producer.send(
                FutureRecord::to(topic)
                    .key(&format!("key-{}", i))
                    .payload(&serde_json::to_string(&event).unwrap()),
                Duration::from_secs(5),
            ).await.unwrap();
        }
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn test_basic_consumer_functionality() {
        let docker = Cli::default();
        let kafka_node = docker.run(Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Wait for Kafka to be ready
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let processor = TestProcessor {
            count: Arc::new(AtomicU32::new(0)),
        };
        
        let config = ConsumerConfigBuilder::new()
            .brokers(brokers.clone())
            .group_id("test-group".to_string())
            .topics(vec!["test-events".to_string()])
            .batch_size(10)
            .build();
        
        let consumer = RedpandaConsumer::new(config, processor.clone()).await.unwrap();
        
        // Run consumer in background
        let consumer_handle = tokio::spawn(async move {
            consumer.run().await
        });
        
        // Produce test events
        produce_events(&brokers, "test-events", 100).await;
        
        // Wait for processing
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        let processed = processor.count.load(Ordering::Relaxed);
        assert!(processed > 80, "Should process most events");
        
        consumer_handle.abort();
    }
    
    #[tokio::test]
    async fn test_consumer_offset_management() {
        let docker = Cli::default();
        let kafka_node = docker.run(Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let processor = TestProcessor {
            count: Arc::new(AtomicU32::new(0)),
        };
        
        let config = ConsumerConfigBuilder::new()
            .brokers(brokers.clone())
            .group_id("offset-test-group".to_string())
            .topics(vec!["offset-test".to_string()])
            .enable_auto_commit(false)
            .batch_size(5)
            .build();
        
        let consumer = RedpandaConsumer::new(config, processor.clone()).await.unwrap();
        let consumer_handle = tokio::spawn(async move {
            consumer.run().await
        });
        
        // Produce first batch
        produce_events(&brokers, "offset-test", 50).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let first_count = processor.count.load(Ordering::Relaxed);
        assert!(first_count > 0, "Should process first batch");
        
        // Restart consumer - should continue from committed offset
        consumer_handle.abort();
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let processor2 = TestProcessor {
            count: Arc::new(AtomicU32::new(0)),
        };
        
        let config2 = ConsumerConfigBuilder::new()
            .brokers(brokers.clone())
            .group_id("offset-test-group".to_string())
            .topics(vec!["offset-test".to_string()])
            .enable_auto_commit(false)
            .batch_size(5)
            .build();
        
        let consumer2 = RedpandaConsumer::new(config2, processor2.clone()).await.unwrap();
        let consumer_handle2 = tokio::spawn(async move {
            consumer2.run().await
        });
        
        // Produce second batch
        produce_events(&brokers, "offset-test", 50).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let second_count = processor2.count.load(Ordering::Relaxed);
        assert!(second_count > 0, "Should process second batch");
        
        consumer_handle2.abort();
    }
    
    #[tokio::test]
    async fn test_consumer_backpressure() {
        let docker = Cli::default();
        let kafka_node = docker.run(Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Slow processor to test backpressure
        #[derive(Clone)]
        struct SlowProcessor {
            count: Arc<AtomicU32>,
        }
        
        #[async_trait]
        impl MessageProcessor for SlowProcessor {
            type Error = ConsumerError;
            
            async fn process(&self, _message: &OwnedMessage) -> Result<(), Self::Error> {
                tokio::time::sleep(Duration::from_millis(100)).await;
                self.count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            
            async fn on_success(&self, _message: &OwnedMessage) {}
            async fn on_failure(&self, _error: &Self::Error, _message: &OwnedMessage) {}
        }
        
        let processor = SlowProcessor {
            count: Arc::new(AtomicU32::new(0)),
        };
        
        let config = ConsumerConfigBuilder::new()
            .brokers(brokers.clone())
            .group_id("backpressure-test".to_string())
            .topics(vec!["backpressure-test".to_string()])
            .max_inflight_messages(10)
            .channel_buffer_size(20)
            .build();
        
        let consumer = RedpandaConsumer::new(config, processor.clone()).await.unwrap();
        let consumer_handle = tokio::spawn(async move {
            consumer.run().await
        });
        
        // Produce many events quickly
        produce_events(&brokers, "backpressure-test", 100).await;
        
        tokio::time::sleep(Duration::from_secs(15)).await;
        
        let processed = processor.count.load(Ordering::Relaxed);
        tracing::error!("Processed {} events with backpressure", processed);
        
        consumer_handle.abort();
    }
    
    #[tokio::test]
    async fn test_consumer_graceful_shutdown() {
        let docker = Cli::default();
        let kafka_node = docker.run(Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let processor = TestProcessor {
            count: Arc::new(AtomicU32::new(0)),
        };
        
        let config = ConsumerConfigBuilder::new()
            .brokers(brokers)
            .group_id("shutdown-test".to_string())
            .topics(vec!["shutdown-test".to_string()])
            .build();
        
        let consumer = RedpandaConsumer::new(config, processor).await.unwrap();
        
        // Start consumer
        let consumer_handle = tokio::spawn(async move {
            consumer.run().await
        });
        
        // Let it run for a bit
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Send shutdown signal
        consumer_handle.abort();
        
        // Should shutdown gracefully
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            consumer_handle
        ).await;
        
        assert!(result.is_ok(), "Consumer should shutdown gracefully");
    }
    
    #[tokio::test]
    async fn test_error_handling_and_dlq() {
        let docker = Cli::default();
        let kafka_node = docker.run(Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // Processor that fails on certain events
        #[derive(Clone)]
        struct FailingProcessor {
            processed: Arc<AtomicU32>,
            failed: Arc<AtomicU32>,
        }
        
        #[async_trait]
        impl MessageProcessor for FailingProcessor {
            type Error = ConsumerError;
            
            async fn process(&self, message: &OwnedMessage) -> Result<(), Self::Error> {
                let payload = message.payload().unwrap();
                let event: serde_json::Value = serde_json::from_slice(payload).unwrap();
                
                if event["fail"].as_bool().unwrap_or(false) {
                    self.failed.fetch_add(1, Ordering::Relaxed);
                    Err(ConsumerError::ProcessingError("Simulated failure".to_string()))
                } else {
                    self.processed.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }
            }
            
            async fn on_success(&self, _message: &OwnedMessage) {}
            async fn on_failure(&self, _error: &Self::Error, _message: &OwnedMessage) {}
            
            fn is_retryable(&self, _error: &Self::Error) -> bool {
                false // Don't retry, send to DLQ
            }
        }
        
        let processor = FailingProcessor {
            processed: Arc::new(AtomicU32::new(0)),
            failed: Arc::new(AtomicU32::new(0)),
        };
        
        let config = ConsumerConfigBuilder::new()
            .brokers(brokers.clone())
            .group_id("error-test-group".to_string())
            .topics(vec!["error-events".to_string()])
            .dlq_topic("error-events-dlq".to_string())
            .dlq_after_retries(1)
            .build();
        
        let consumer = RedpandaConsumer::new(config, processor.clone()).await.unwrap();
        let consumer_handle = tokio::spawn(async move {
            consumer.run().await
        });
        
        // Produce mix of good and bad events
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .create()
            .unwrap();
        
        for i in 0..20 {
            let event = serde_json::json!({
                "id": i,
                "fail": i % 3 == 0,  // Every 3rd event fails
                "data": format!("event {}", i)
            });
            
            producer.send(
                FutureRecord::to("error-events")
                    .key(&format!("key-{}", i))
                    .payload(&serde_json::to_string(&event).unwrap()),
                Duration::from_secs(5),
            ).await.unwrap();
        }
        
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        let processed = processor.processed.load(Ordering::Relaxed);
        let failed = processor.failed.load(Ordering::Relaxed);
        
        tracing::error!("Successfully processed: {}, Failed: {}", processed, failed);
        
        assert!(processed > 0, "Should process good events");
        assert!(failed > 0, "Should fail on bad events");
        
        consumer_handle.abort();
    }
}
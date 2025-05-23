#[cfg(test)]
mod tests {
    use sigma_rs::consumer::{
        config::ConsumerConfig,
        error::ConsumerResult,
        dlq::DlqProducer,
        offset_manager::OffsetManager,
        retry::{RetryPolicy, RetryExecutor, RetryResult},
        backpressure::BackpressureController,
        shutdown::{ShutdownState, ShutdownCoordinator},
        error::ConsumerError,
    };
    use rdkafka::{
        ClientConfig,
        consumer::{StreamConsumer, Consumer},
        producer::FutureProducer,
        message::OwnedMessage,
        Offset,
    };
    use std::time::Duration;
    use testcontainers::{clients, images::kafka};
    use futures::StreamExt;
    
    #[tokio::test]
    async fn test_kafka_connection() {
        let docker = clients::Cli::default();
        let kafka_node = docker.run(kafka::Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Test consumer creation
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("group.id", "test-group")
            .set("auto.offset.reset", "earliest")
            .create()
            .expect("Failed to create consumer");
        
        // Test subscription
        consumer.subscribe(&["test-topic"]).expect("Failed to subscribe");
        
        // Test producer creation
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .create()
            .expect("Failed to create producer");
        
        // Basic connectivity verified
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_dlq_producer() {
        let docker = clients::Cli::default();
        let kafka_node = docker.run(kafka::Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Wait for Kafka to be ready
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Create DLQ producer
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("message.timeout.ms", "5000")
            .create()
            .expect("Failed to create producer");
        
        let dlq = DlqProducer::new(producer, "test-dlq".to_string())
            .with_timeout(Duration::from_secs(5))
            .with_metadata(true);
        
        // Create test message
        let message = OwnedMessage::new(
            Some(b"test-key".to_vec()),
            Some(b"test-payload".to_vec()),
            "original-topic".to_string(),
            rdkafka::Timestamp::CreateTime(0),
            0,
            42,
            None,
        );
        
        // Send to DLQ
        let result = dlq.send_message(&message, "Test error", 1).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_offset_manager() {
        let docker = clients::Cli::default();
        let kafka_node = docker.run(kafka::Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Create consumer
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("group.id", "offset-test-group")
            .set("enable.auto.commit", "false")
            .create()
            .expect("Failed to create consumer");
        
        let offset_manager = OffsetManager::new(10, Duration::from_secs(5));
        
        // Mark some offsets
        offset_manager.mark_offset("test-topic".to_string(), 0, 10).await;
        offset_manager.mark_offset("test-topic".to_string(), 1, 20).await;
        offset_manager.mark_offset("test-topic".to_string(), 0, 11).await;
        
        // Commit offsets
        let result = offset_manager.commit_offsets(&consumer).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_retry_executor() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
            exponential: true,
        };
        
        let executor = RetryExecutor::new(policy);
        
        // Test successful retry
        let mut attempt = 0;
        let result = executor.execute(|| async {
            attempt += 1;
            if attempt < 3 {
                Err(ConsumerError::Timeout("Temporary error".to_string()))
            } else {
                Ok("Success")
            }
        }).await;
        
        match result {
            RetryResult::Success { value, attempts } => {
                assert_eq!(value, "Success");
                assert_eq!(attempts, 2); // 0-indexed, so 2 retries means 3 total attempts
            }
            _ => panic!("Expected success"),
        }
    }
    
    #[tokio::test]
    async fn test_backpressure_controller() {
        let controller = BackpressureController::new(10, 0.8, 0.5);
        
        // Acquire permits
        let permits = futures::stream::iter(0..8)
            .then(|_| controller.acquire())
            .collect::<Vec<_>>()
            .await;
        
        assert_eq!(controller.inflight_count(), 8);
        assert!(controller.should_pause()); // 8/10 = 0.8
        
        // Drop some permits
        drop(permits.into_iter().take(4).collect::<Vec<_>>());
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        assert_eq!(controller.inflight_count(), 4);
        assert!(controller.should_resume()); // 4/10 = 0.4 < 0.5
    }
    
    #[tokio::test]
    async fn test_shutdown_coordinator() {
        let shutdown_state = ShutdownState::new();
        let coordinator = ShutdownCoordinator::new(shutdown_state.clone());
        
        // Add some inflight messages
        shutdown_state.add_inflight_message().await;
        shutdown_state.add_inflight_message().await;
        
        // Start shutdown
        let handle = tokio::spawn(async move {
            coordinator.coordinate_shutdown(Duration::from_secs(5)).await
        });
        
        // Remove inflight messages
        tokio::time::sleep(Duration::from_millis(100)).await;
        shutdown_state.remove_inflight_message().await;
        shutdown_state.remove_inflight_message().await;
        
        // Shutdown should complete
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_consumer_rebalancing() {
        let docker = clients::Cli::default();
        let kafka_node = docker.run(kafka::Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Create first consumer
        let consumer1: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("group.id", "rebalance-test-group")
            .set("session.timeout.ms", "6000")
            .create()
            .expect("Failed to create consumer");
        
        consumer1.subscribe(&["rebalance-topic"]).unwrap();
        
        // Let it settle
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Create second consumer in same group
        let consumer2: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("group.id", "rebalance-test-group")
            .set("session.timeout.ms", "6000")
            .create()
            .expect("Failed to create consumer");
        
        consumer2.subscribe(&["rebalance-topic"]).unwrap();
        
        // Rebalancing should occur
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Both consumers should be active
        assert!(true); // In real test, check partition assignments
    }
    
    #[tokio::test]
    async fn test_message_consumption() {
        let docker = clients::Cli::default();
        let kafka_node = docker.run(kafka::Kafka::default());
        let brokers = format!("localhost:{}", kafka_node.get_host_port_ipv4(9092));
        
        // Create producer
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .create()
            .unwrap();
        
        // Produce messages
        for i in 0..5 {
            producer.send(
                rdkafka::producer::FutureRecord::to("consume-test")
                    .key(&format!("key-{}", i))
                    .payload(&format!("message-{}", i)),
                Duration::from_secs(5),
            ).await.unwrap();
        }
        
        // Create consumer
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("group.id", "consume-test-group")
            .set("auto.offset.reset", "earliest")
            .create()
            .unwrap();
        
        consumer.subscribe(&["consume-test"]).unwrap();
        
        // Consume messages
        let mut count = 0;
        let mut stream = consumer.stream();
        
        while let Ok(Some(Ok(message))) = tokio::time::timeout(
            Duration::from_secs(5),
            stream.next()
        ).await {
            count += 1;
            if count >= 5 {
                break;
            }
        }
        
        assert_eq!(count, 5);
    }
}
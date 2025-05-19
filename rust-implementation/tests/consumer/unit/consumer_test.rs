use sigma_rs::consumer::{
    consumer::RedpandaConsumer,
    config::ConsumerConfig,
    processor::MessageProcessor,
    error::{ConsumerError, ConsumerResult},
    metrics::ConsumerMetrics,
    retry::RetryPolicy,
};
use mockall::*;
use rdkafka::{
    Message,
    message::{OwnedMessage, BorrowedMessage},
    producer::{FutureProducer, FutureRecord},
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    topic_partition_list::TopicPartitionList,
};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, watch};
use tokio::time::{Duration, timeout};
use async_trait::async_trait;
use pretty_assertions::assert_eq;

// Mock processor implementation
mock! {
    Processor {}
    
    #[async_trait]
    impl MessageProcessor for Processor {
        type Error = ConsumerError;
        
        async fn process(&self, message: &OwnedMessage) -> Result<(), Self::Error>;
        async fn on_success(&self, message: &OwnedMessage);
        async fn on_failure(&self, error: &Self::Error, message: &OwnedMessage);
        fn is_retryable(&self, error: &Self::Error) -> bool;
    }
}

// Test data helper
fn create_test_message(topic: &str, partition: i32, offset: i64) -> OwnedMessage {
    OwnedMessage::new(
        Some(b"test-key".to_vec()),
        Some(b"test-payload".to_vec()),
        topic.to_string(),
        rdkafka::Timestamp::CreateTime(0),
        partition,
        offset,
        None,
    )
}

fn create_test_config() -> ConsumerConfig {
    ConsumerConfig {
        brokers: "localhost:9092".to_string(),
        group_id: "test-group".to_string(),
        topics: vec!["test-topic".to_string()],
        session_timeout_ms: 6000,
        max_poll_interval_ms: 300000,
        enable_auto_commit: false,
        auto_commit_interval_ms: 5000,
        auto_offset_reset: "latest".to_string(),
        batch_size: 10,
        processing_timeout: Duration::from_secs(5),
        retry_policy: RetryPolicy::default(),
        dlq_topic: Some("test-dlq".to_string()),
        dlq_after_retries: 3,
        channel_buffer_size: 100,
        max_inflight_messages: 50,
        pause_threshold: 0.8,
        resume_threshold: 0.5,
        metrics_interval: Duration::from_secs(60),
        enable_detailed_metrics: false,
        kafka_properties: HashMap::new(),
    }
}

#[tokio::test]
async fn test_consumer_creation_success() {
    let config = create_test_config();
    let mut mock_processor = MockProcessor::new();
    
    // Set up expectations
    mock_processor
        .expect_process()
        .times(0);
    
    // We can't easily test consumer creation due to Kafka dependencies
    // This would require integration tests with testcontainers
    // For now, just validate the config
    assert!(config.validate().is_ok());
}

#[test]
fn test_invalid_config_fails() {
    let mut config = create_test_config();
    config.brokers = String::new(); // Invalid empty brokers
    
    assert!(config.validate().is_err());
}

// Test ProcessingTask struct
#[test]
fn test_processing_task_creation() {
    use std::time::Instant;
    
    let message = create_test_message("test", 0, 42);
    let task = super::ProcessingTask {
        message: message.clone(),
        attempt: 0,
        start_time: Instant::now(),
    };
    
    assert_eq!(task.attempt, 0);
    assert_eq!(task.message.topic(), "test");
    assert_eq!(task.message.partition(), 0);
    assert_eq!(task.message.offset(), 42);
}

// Test message processors
#[tokio::test]
async fn test_processor_success_flow() {
    let mut mock_processor = MockProcessor::new();
    let message = create_test_message("test", 0, 42);
    
    mock_processor
        .expect_process()
        .withf(|msg: &OwnedMessage| {
            msg.topic() == "test" && msg.partition() == 0 && msg.offset() == 42
        })
        .times(1)
        .returning(|_| Ok(()));
    
    mock_processor
        .expect_on_success()
        .withf(|msg: &OwnedMessage| {
            msg.topic() == "test" && msg.partition() == 0 && msg.offset() == 42
        })
        .times(1)
        .return_const(());
    
    // Execute
    let result = mock_processor.process(&message).await;
    assert!(result.is_ok());
    mock_processor.on_success(&message).await;
}

#[tokio::test]
async fn test_processor_failure_flow() {
    let mut mock_processor = MockProcessor::new();
    let message = create_test_message("test", 0, 42);
    let error = ConsumerError::ProcessingError("Test error".to_string());
    
    mock_processor
        .expect_process()
        .times(1)
        .returning(|_| Err(ConsumerError::ProcessingError("Test error".to_string())));
    
    mock_processor
        .expect_on_failure()
        .times(1)
        .return_const(());
    
    // Execute
    let result = mock_processor.process(&message).await;
    assert!(result.is_err());
    mock_processor.on_failure(&result.unwrap_err(), &message).await;
}

#[test]
fn test_processor_retryable_logic() {
    let mut mock_processor = MockProcessor::new();
    
    mock_processor
        .expect_is_retryable()
        .with(predicate::function(|e: &ConsumerError| {
            matches!(e, ConsumerError::ProcessingError(_))
        }))
        .times(1)
        .returning(|_| true);
    
    mock_processor
        .expect_is_retryable()
        .with(predicate::function(|e: &ConsumerError| {
            matches!(e, ConsumerError::ParseError(_))
        }))
        .times(1)
        .returning(|_| false);
    
    let retryable_error = ConsumerError::ProcessingError("Retryable".to_string());
    let non_retryable_error = ConsumerError::ParseError("Non-retryable".to_string());
    
    assert!(mock_processor.is_retryable(&retryable_error));
    assert!(!mock_processor.is_retryable(&non_retryable_error));
}

// Test retry logic
#[tokio::test]
async fn test_process_message_with_retry_success() {
    let mut mock_processor = MockProcessor::new();
    let message = create_test_message("test", 0, 42);
    let config = create_test_config();
    let mut attempt = 0u32;
    
    // First attempt fails, second succeeds
    mock_processor
        .expect_process()
        .times(2)
        .returning(|_| {
            static mut CALL_COUNT: u32 = 0;
            unsafe {
                CALL_COUNT += 1;
                if CALL_COUNT == 1 {
                    Err(ConsumerError::ProcessingError("Temporary error".to_string()))
                } else {
                    Ok(())
                }
            }
        });
    
    mock_processor
        .expect_is_retryable()
        .times(1)
        .returning(|_| true);
    
    let result = super::process_message_with_retry(
        &mock_processor,
        &message,
        &config,
        &mut attempt,
    ).await;
    
    assert!(result.is_ok());
    assert!(attempt > 0);
}

#[tokio::test]
async fn test_process_message_with_retry_max_attempts() {
    let mut mock_processor = MockProcessor::new();
    let message = create_test_message("test", 0, 42);
    let mut config = create_test_config();
    config.retry_policy.max_retries = 2;
    let mut attempt = 0u32;
    
    // Always fail
    mock_processor
        .expect_process()
        .times(3) // Initial + 2 retries
        .returning(|_| Err(ConsumerError::ProcessingError("Persistent error".to_string())));
    
    mock_processor
        .expect_is_retryable()
        .times(3)
        .returning(|_| true);
    
    let result = super::process_message_with_retry(
        &mock_processor,
        &message,
        &config,
        &mut attempt,
    ).await;
    
    assert!(result.is_err());
    assert_eq!(attempt, 3);
}

// Test channel communication
#[tokio::test]
async fn test_processing_channel() {
    let (tx, mut rx) = mpsc::channel::<super::ProcessingTask>(10);
    
    let message = create_test_message("test", 0, 42);
    let task = super::ProcessingTask {
        message: message.clone(),
        attempt: 0,
        start_time: std::time::Instant::now(),
    };
    
    // Send task
    assert!(tx.send(task).await.is_ok());
    
    // Receive task
    let received = rx.recv().await;
    assert!(received.is_some());
    
    let received_task = received.unwrap();
    assert_eq!(received_task.attempt, 0);
    assert_eq!(received_task.message.topic(), "test");
}

// Test shutdown coordination
#[tokio::test]
async fn test_shutdown_signal() {
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    
    // Initial state
    assert!(!*shutdown_rx.borrow());
    
    // Send shutdown signal
    assert!(shutdown_tx.send(true).is_ok());
    
    // Check signal received
    assert!(shutdown_rx.changed().await.is_ok());
    assert!(*shutdown_rx.borrow());
}

// Test configuration validation in consumer
#[test]
fn test_consumer_config_validation() {
    let valid_config = create_test_config();
    assert!(valid_config.validate().is_ok());
    
    let mut invalid_config = create_test_config();
    invalid_config.topics = vec![];
    assert!(invalid_config.validate().is_err());
}

// Test DLQ topic configuration
#[test]
fn test_dlq_configuration() {
    let config_with_dlq = create_test_config();
    assert_eq!(config_with_dlq.dlq_topic, Some("test-dlq".to_string()));
    assert_eq!(config_with_dlq.dlq_after_retries, 3);
    
    let mut config_without_dlq = create_test_config();
    config_without_dlq.dlq_topic = None;
    assert_eq!(config_without_dlq.dlq_topic, None);
}

// Test backpressure thresholds
#[test]
fn test_backpressure_configuration() {
    let config = create_test_config();
    assert_eq!(config.pause_threshold, 0.8);
    assert_eq!(config.resume_threshold, 0.5);
    assert_eq!(config.max_inflight_messages, 50);
    
    // Validate thresholds
    assert!(config.pause_threshold > config.resume_threshold);
}

// Test metrics configuration
#[test]
fn test_metrics_configuration() {
    let config = create_test_config();
    assert_eq!(config.metrics_interval, Duration::from_secs(60));
    assert!(!config.enable_detailed_metrics);
}

// Test error conversion
#[test]
fn test_consumer_error_types() {
    let config_err = ConsumerError::ConfigError("Bad config".to_string());
    assert!(matches!(config_err, ConsumerError::ConfigError(_)));
    
    let connection_err = ConsumerError::ConnectionError("No connection".to_string());
    assert!(matches!(connection_err, ConsumerError::ConnectionError(_)));
}

// Test message detachment
#[test]
fn test_message_ownership() {
    let message = create_test_message("test", 0, 42);
    let topic = message.topic().to_string();
    let partition = message.partition();
    let offset = message.offset();
    
    // Simulate detaching (moving ownership)
    let detached = message;
    
    assert_eq!(detached.topic(), topic);
    assert_eq!(detached.partition(), partition);
    assert_eq!(detached.offset(), offset);
}

// Test panic safety
#[test]
fn test_consumer_types_are_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    
    assert_send_sync::<ConsumerConfig>();
    assert_send_sync::<Arc<ConsumerMetrics>>();
    // RedpandaConsumer would require a concrete MessageProcessor type
}

// Test adaptive backpressure config
#[test]
fn test_adaptive_backpressure_config() {
    use sigma_rs::consumer::backpressure::AdaptiveBackpressureConfig;
    
    let config = AdaptiveBackpressureConfig {
        initial_inflight: 100,
        min_inflight: 10,
        max_inflight: 1000,
        pause_threshold: 0.8,
        resume_threshold: 0.5,
        adjustment_interval: Duration::from_secs(30),
        target_latency: Duration::from_millis(100),
        target_success_rate: 0.95,
    };
    
    assert!(config.max_inflight > config.min_inflight);
    assert!(config.pause_threshold > config.resume_threshold);
    assert!(config.target_success_rate <= 1.0);
}
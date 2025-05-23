use sigma_rs::consumer::error::{ConsumerError, ConsumerResult, ErrorSeverity};
use rdkafka::error::KafkaError;
use std::io;
use tokio::sync::{mpsc, oneshot};
use test_case::test_case;

#[test]
fn test_error_creation() {
    // Test each error variant creation
    let config_err = ConsumerError::ConfigError("Invalid config".to_string());
    let connection_err = ConsumerError::ConnectionError("Connection failed".to_string());
    let parse_err = ConsumerError::ParseError("Parse failed".to_string());
    let processing_err = ConsumerError::ProcessingError("Processing failed".to_string());
    let offset_err = ConsumerError::OffsetError("Offset error".to_string());
    let dlq_err = ConsumerError::DlqError("DLQ error".to_string());
    let timeout_err = ConsumerError::Timeout("Timeout occurred".to_string());
    let backpressure_err = ConsumerError::Backpressure("Too much pressure".to_string());
    let channel_err = ConsumerError::ChannelError("Channel closed".to_string());
    let shutdown_err = ConsumerError::ShutdownError("Shutdown failed".to_string());
    let generic_err = ConsumerError::Generic("Generic error".to_string());
    
    // Test display formatting
    assert!(format!("{}", config_err).contains("Configuration error"));
    assert!(format!("{}", connection_err).contains("Connection error"));
    assert!(format!("{}", parse_err).contains("Parse error"));
    assert!(format!("{}", processing_err).contains("Processing error"));
    assert!(format!("{}", offset_err).contains("Offset error"));
    assert!(format!("{}", dlq_err).contains("DLQ error"));
    assert!(format!("{}", timeout_err).contains("Timeout"));
    assert!(format!("{}", backpressure_err).contains("Backpressure"));
    assert!(format!("{}", channel_err).contains("Channel error"));
    assert!(format!("{}", shutdown_err).contains("Shutdown error"));
    assert!(format!("{}", generic_err).contains("Consumer error"));
}

#[test]
fn test_from_conversions() {
    // Test IO error conversion
    let io_err = io::Error::new(io::ErrorKind::NotFound, "File not found");
    let consumer_err = ConsumerError::from(io_err);
    assert!(matches!(consumer_err, ConsumerError::IoError(_)));
    
    // Test Kafka error conversion
    let kafka_err = KafkaError::NoError;
    let consumer_err = ConsumerError::from(kafka_err);
    assert!(matches!(consumer_err, ConsumerError::KafkaError(_)));
}

#[test_case(ConsumerError::ConnectionError("".to_string()), true ; "connection error is retryable")]
#[test_case(ConsumerError::Timeout("".to_string()), true ; "timeout is retryable")]
#[test_case(ConsumerError::IoError(io::Error::new(io::ErrorKind::Other, "")), true ; "io error is retryable")]
#[test_case(ConsumerError::ProcessingError("".to_string()), true ; "processing error is retryable")]
#[test_case(ConsumerError::ParseError("".to_string()), false ; "parse error is not retryable")]
#[test_case(ConsumerError::ConfigError("".to_string()), false ; "config error is not retryable")]
#[test_case(ConsumerError::DlqError("".to_string()), false ; "dlq error is not retryable")]
fn test_is_retryable(error: ConsumerError, expected: bool) {
    assert_eq!(error.is_retryable(), expected);
}

#[test]
fn test_kafka_error_retryability() {
    // Test specific Kafka error types
    let non_retryable = ConsumerError::KafkaError(
        KafkaError::MessageConsumption(rdkafka::types::RDKafkaErrorCode::InvalidGroupId)
    );
    assert!(!non_retryable.is_retryable());
    
    let retryable = ConsumerError::KafkaError(KafkaError::NoError);
    assert!(retryable.is_retryable());
}

#[test_case(ConsumerError::ConfigError("".to_string()), ErrorSeverity::Fatal ; "config error is fatal")]
#[test_case(ConsumerError::ParseError("".to_string()), ErrorSeverity::Warning ; "parse error is warning")]
#[test_case(ConsumerError::ProcessingError("".to_string()), ErrorSeverity::Error ; "processing error is error")]
#[test_case(ConsumerError::ConnectionError("".to_string()), ErrorSeverity::Error ; "connection error is error")]
#[test_case(ConsumerError::Timeout("".to_string()), ErrorSeverity::Warning ; "timeout is warning")]
#[test_case(ConsumerError::Generic("".to_string()), ErrorSeverity::Error ; "generic error is error")]
fn test_error_severity(error: ConsumerError, expected: ErrorSeverity) {
    assert_eq!(error.severity(), expected);
}

#[test]
fn test_error_display_formatting() {
    let errors = vec![
        (ConsumerError::ConfigError("bad config".to_string()), "Configuration error: bad config"),
        (ConsumerError::ConnectionError("no connection".to_string()), "Connection error: no connection"),
        (ConsumerError::ParseError("invalid json".to_string()), "Parse error: invalid json"),
        (ConsumerError::ProcessingError("failed to process".to_string()), "Processing error: failed to process"),
        (ConsumerError::OffsetError("invalid offset".to_string()), "Offset error: invalid offset"),
        (ConsumerError::DlqError("dlq failed".to_string()), "DLQ error: dlq failed"),
        (ConsumerError::Timeout("30s timeout".to_string()), "Timeout: 30s timeout"),
        (ConsumerError::Backpressure("queue full".to_string()), "Backpressure: queue full"),
        (ConsumerError::ChannelError("channel closed".to_string()), "Channel error: channel closed"),
        (ConsumerError::ShutdownError("graceful shutdown failed".to_string()), "Shutdown error: graceful shutdown failed"),
        (ConsumerError::Generic("unknown error".to_string()), "Consumer error: unknown error"),
    ];
    
    for (error, expected) in errors {
        assert_eq!(format!("{}", error), expected);
    }
}

#[test]
fn test_error_debug_impl() {
    let error = ConsumerError::ConfigError("test".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ConfigError"));
    assert!(debug_str.contains("test"));
}

#[tokio::test]
async fn test_channel_send_error_conversion() {
    let (tx, _rx) = mpsc::channel::<String>(1);
    drop(_rx); // Drop receiver to cause send error
    
    let result = tx.send("test".to_string()).await;
    assert!(result.is_err());
    
    let consumer_err: ConsumerError = result.err().unwrap().into();
    assert!(matches!(consumer_err, ConsumerError::ChannelError(_)));
    assert!(format!("{}", consumer_err).contains("Failed to send"));
}

#[tokio::test]
async fn test_oneshot_recv_error_conversion() {
    let (tx, rx) = oneshot::channel::<String>();
    drop(tx); // Drop sender to cause recv error
    
    let result = rx.await;
    assert!(result.is_err());
    
    let consumer_err: ConsumerError = result.err().unwrap().into();
    assert!(matches!(consumer_err, ConsumerError::ChannelError(_)));
    assert!(format!("{}", consumer_err).contains("Failed to receive"));
}

#[test]
fn test_result_type_alias() {
    // Test that ConsumerResult works correctly
    let ok_result: ConsumerResult<i32> = Ok(42);
    assert!(ok_result.is_ok());
    assert_eq!(ok_result.unwrap(), 42);
    
    let err_result: ConsumerResult<i32> = Err(ConsumerError::Generic("test".to_string()));
    assert!(err_result.is_err());
    assert!(matches!(err_result.unwrap_err(), ConsumerError::Generic(_)));
}

#[test]
fn test_error_chaining() {
    // Test that errors can be properly chained
    let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
    let consumer_err = ConsumerError::from(io_err);
    
    // Verify error source chain
    let error_chain = format!("{:?}", consumer_err);
    assert!(error_chain.contains("PermissionDenied"));
    assert!(error_chain.contains("Access denied"));
}

#[test]
fn test_error_severity_ordering() {
    // Ensure severity levels can be compared
    assert!(ErrorSeverity::Warning < ErrorSeverity::Error);
    assert!(ErrorSeverity::Error < ErrorSeverity::Fatal);
    assert_eq!(ErrorSeverity::Warning, ErrorSeverity::Warning);
}

#[test]
fn test_error_is_send_sync() {
    // Ensure error types are Send + Sync for async usage
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ConsumerError>();
    assert_send_sync::<ErrorSeverity>();
}

// Property-based test for error formatting
#[test]
fn test_error_messages_non_empty() {
    let errors = vec![
        ConsumerError::ConfigError("".to_string()),
        ConsumerError::ConnectionError("".to_string()),
        ConsumerError::ParseError("".to_string()),
        ConsumerError::ProcessingError("".to_string()),
        ConsumerError::OffsetError("".to_string()),
        ConsumerError::DlqError("".to_string()),
        ConsumerError::Timeout("".to_string()),
        ConsumerError::Backpressure("".to_string()),
        ConsumerError::ChannelError("".to_string()),
        ConsumerError::ShutdownError("".to_string()),
        ConsumerError::Generic("".to_string()),
    ];
    
    for error in errors {
        let display = format!("{}", error);
        assert!(!display.is_empty());
        assert!(display.contains(":"));
    }
}

#[test]
fn test_error_pattern_matching() {
    let error = ConsumerError::ConfigError("test".to_string());
    
    let message = match error {
        ConsumerError::ConfigError(msg) => msg,
        _ => panic!("Expected ConfigError"),
    };
    
    assert_eq!(message, "test");
}

#[test]
fn test_error_severity_exhaustive() {
    // Ensure all error types have a severity
    let errors = vec![
        ConsumerError::ConfigError("".to_string()),
        ConsumerError::ConnectionError("".to_string()),
        ConsumerError::ParseError("".to_string()),
        ConsumerError::ProcessingError("".to_string()),
        ConsumerError::OffsetError("".to_string()),
        ConsumerError::DlqError("".to_string()),
        ConsumerError::Timeout("".to_string()),
        ConsumerError::Backpressure("".to_string()),
        ConsumerError::ChannelError("".to_string()),
        ConsumerError::ShutdownError("".to_string()),
        ConsumerError::Generic("".to_string()),
        ConsumerError::IoError(io::Error::new(io::ErrorKind::Other, "")),
        ConsumerError::KafkaError(KafkaError::NoError),
    ];
    
    for error in errors {
        // This should not panic
        let _ = error.severity();
    }
}
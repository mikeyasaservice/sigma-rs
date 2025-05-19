//! Consumer error types

use thiserror::Error;
use std::io;

/// Result type for consumer operations
pub type ConsumerResult<T> = Result<T, ConsumerError>;

/// Consumer error types
#[derive(Error, Debug)]
pub enum ConsumerError {
    /// Kafka client errors
    #[error("Kafka error: {0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    /// Connection errors
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    /// Message parsing errors
    #[error("Parse error: {0}")]
    ParseError(String),
    
    /// Processing errors
    #[error("Processing error: {0}")]
    ProcessingError(String),
    
    /// Offset management errors
    #[error("Offset error: {0}")]
    OffsetError(String),
    
    /// DLQ errors
    #[error("DLQ error: {0}")]
    DlqError(String),
    
    /// Timeout errors
    #[error("Timeout: {0}")]
    Timeout(String),
    
    /// Backpressure errors
    #[error("Backpressure: {0}")]
    Backpressure(String),
    
    /// IO errors
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
    
    /// Channel errors
    #[error("Channel error: {0}")]
    ChannelError(String),
    
    /// Shutdown errors
    #[error("Shutdown error: {0}")]
    ShutdownError(String),
    
    /// Generic errors
    #[error("Consumer error: {0}")]
    Generic(String),
}

impl ConsumerError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            ConsumerError::KafkaError(e) => {
                // Check for retryable Kafka errors
                !matches!(e, rdkafka::error::KafkaError::MessageConsumption(_))
            },
            ConsumerError::ConnectionError(_) => true,
            ConsumerError::Timeout(_) => true,
            ConsumerError::IoError(_) => true,
            ConsumerError::ProcessingError(_) => true,
            ConsumerError::ParseError(_) => false,
            ConsumerError::ConfigError(_) => false,
            ConsumerError::DlqError(_) => false,
            _ => false,
        }
    }
    
    /// Get error severity
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ConsumerError::ConfigError(_) => ErrorSeverity::Fatal,
            ConsumerError::ParseError(_) => ErrorSeverity::Warning,
            ConsumerError::ProcessingError(_) => ErrorSeverity::Error,
            ConsumerError::ConnectionError(_) => ErrorSeverity::Error,
            ConsumerError::Timeout(_) => ErrorSeverity::Warning,
            _ => ErrorSeverity::Error,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Warnings that don't stop processing
    Warning,
    /// Errors that may be retried
    Error,
    /// Fatal errors that stop the consumer
    Fatal,
}

/// Convert from channel send errors
impl<T> From<tokio::sync::mpsc::error::SendError<T>> for ConsumerError {
    fn from(err: tokio::sync::mpsc::error::SendError<T>) -> Self {
        ConsumerError::ChannelError(format!("Failed to send: {}", err))
    }
}

/// Convert from channel receive errors
impl From<tokio::sync::oneshot::error::RecvError> for ConsumerError {
    fn from(err: tokio::sync::oneshot::error::RecvError) -> Self {
        ConsumerError::ChannelError(format!("Failed to receive: {}", err))
    }
}
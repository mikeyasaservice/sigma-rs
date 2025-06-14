//! Message processor trait and implementations

use async_trait::async_trait;
use rdkafka::message::OwnedMessage;
use std::fmt::Debug;

/// Trait for processing Kafka messages
#[async_trait]
pub trait MessageProcessor: Send + Sync + 'static {
    /// Error type for processing
    type Error: std::error::Error + Send + Sync + Debug;

    /// Process a single message
    async fn process(&self, message: &OwnedMessage) -> Result<(), Self::Error>;

    /// Called when a message is successfully processed
    async fn on_success(&self, message: &OwnedMessage);

    /// Called when message processing fails
    async fn on_failure(&self, error: &Self::Error, message: &OwnedMessage);

    /// Optional pre-processing hook
    async fn pre_process(&self, _message: &OwnedMessage) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Optional post-processing hook
    async fn post_process(&self, _message: &OwnedMessage) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Check if error is retryable
    fn is_retryable(&self, _error: &Self::Error) -> bool {
        true
    }

    /// Get processor name for metrics
    fn name(&self) -> &str {
        "MessageProcessor"
    }
}

/// Batch processor trait for processing multiple messages at once
#[async_trait]
pub trait BatchProcessor: Send + Sync + 'static {
    /// Error type for processing
    type Error: std::error::Error + Send + Sync + Debug;

    /// Process a batch of messages
    async fn process_batch(&self, messages: Vec<OwnedMessage>) -> Vec<Result<(), Self::Error>>;

    /// Get maximum batch size
    fn max_batch_size(&self) -> usize {
        100
    }

    /// Get batch timeout
    fn batch_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
}

/// Message processing context
#[derive(Debug, Clone)]
pub struct ProcessingContext {
    /// Message topic
    pub topic: String,
    /// Message partition
    pub partition: i32,
    /// Message offset
    pub offset: i64,
    /// Message key (if present)
    pub key: Option<Vec<u8>>,
    /// Message timestamp
    pub timestamp: Option<i64>,
    /// Processing attempt number
    pub attempt: u32,
    /// Processing start time
    pub start_time: std::time::Instant,
}

impl ProcessingContext {
    /// Create context from a Kafka message
    pub fn from_message(message: &OwnedMessage, attempt: u32) -> Self {
        use rdkafka::Message;
        Self {
            topic: message.topic().to_string(),
            partition: message.partition(),
            offset: message.offset(),
            key: message.key().map(|k| k.to_vec()),
            timestamp: message.timestamp().to_millis(),
            attempt,
            start_time: std::time::Instant::now(),
        }
    }

    /// Get processing duration
    pub fn duration(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

/// Processing result with metrics
#[derive(Debug)]
pub struct ProcessingResult {
    /// Whether processing succeeded
    pub success: bool,
    /// Error if processing failed
    pub error: Option<String>,
    /// Processing duration
    pub duration: std::time::Duration,
    /// Number of attempts
    pub attempts: u32,
    /// Whether message was sent to DLQ
    pub sent_to_dlq: bool,
}

/// Wrapper for adding metrics to any processor
pub struct MetricsProcessor<P: MessageProcessor> {
    inner: P,
    metrics: crate::consumer::metrics::ConsumerMetrics,
}

impl<P: MessageProcessor> MetricsProcessor<P> {
    pub fn new(processor: P, metrics: crate::consumer::metrics::ConsumerMetrics) -> Self {
        Self {
            inner: processor,
            metrics,
        }
    }
}

#[async_trait]
impl<P: MessageProcessor> MessageProcessor for MetricsProcessor<P> {
    type Error = P::Error;

    async fn process(&self, message: &OwnedMessage) -> Result<(), Self::Error> {
        let start = std::time::Instant::now();
        let result = self.inner.process(message).await;
        let duration = start.elapsed();

        // Update metrics
        self.metrics.record_processing_duration(duration);
        if result.is_ok() {
            self.metrics.increment_processed();
        } else {
            self.metrics.increment_failed();
        }

        result
    }

    async fn on_success(&self, message: &OwnedMessage) {
        self.inner.on_success(message).await;
    }

    async fn on_failure(&self, error: &Self::Error, message: &OwnedMessage) {
        self.inner.on_failure(error, message).await;
    }
}

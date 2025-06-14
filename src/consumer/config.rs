//! Consumer configuration structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Redpanda/Kafka consumer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerConfig {
    /// Kafka broker addresses (comma-separated)
    pub brokers: String,

    /// Consumer group ID
    pub group_id: String,

    /// Topics to consume from
    pub topics: Vec<String>,

    /// Session timeout in milliseconds
    pub session_timeout_ms: u32,

    /// Maximum poll interval in milliseconds
    pub max_poll_interval_ms: u32,

    /// Enable auto-commit
    pub enable_auto_commit: bool,

    /// Auto-commit interval in milliseconds
    pub auto_commit_interval_ms: u32,

    /// Offset reset policy (earliest, latest, none)
    pub auto_offset_reset: String,

    /// Batch processing settings
    pub batch_size: usize,

    /// Processing timeout per message
    pub processing_timeout: Duration,

    /// Connection timeout for initial consumer creation
    pub connection_timeout: Duration,

    /// Network operation timeout for ongoing operations
    pub network_timeout: Duration,

    /// Retry policy
    pub retry_policy: crate::consumer::retry::RetryPolicy,

    /// Dead letter queue configuration
    pub dlq_topic: Option<String>,
    pub dlq_after_retries: u32,

    /// Backpressure settings
    pub channel_buffer_size: usize,
    pub max_inflight_messages: usize,
    pub pause_threshold: f64,  // Pause when buffer is this % full
    pub resume_threshold: f64, // Resume when buffer drops to this %

    /// Metrics configuration
    pub metrics_interval: Duration,
    pub enable_detailed_metrics: bool,

    /// Worker configuration
    pub num_workers: usize,

    /// Batch processing configuration
    pub batch_timeout: Duration,
    pub enable_batching: bool,

    /// Additional Kafka properties
    pub kafka_properties: HashMap<String, String>,
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            group_id: "sigma-engine".to_string(),
            topics: vec!["events".to_string()],
            session_timeout_ms: 30000,
            max_poll_interval_ms: 300000,
            enable_auto_commit: false,
            auto_commit_interval_ms: 5000,
            auto_offset_reset: "latest".to_string(),
            batch_size: 100,
            processing_timeout: Duration::from_secs(30),
            connection_timeout: Duration::from_secs(30),
            network_timeout: Duration::from_secs(10),
            retry_policy: crate::consumer::retry::RetryPolicy::default(),
            dlq_topic: None,
            dlq_after_retries: 3,
            channel_buffer_size: 1000,
            max_inflight_messages: 500,
            pause_threshold: 0.8,
            resume_threshold: 0.5,
            metrics_interval: Duration::from_secs(60),
            enable_detailed_metrics: false,
            num_workers: num_cpus::get(),
            batch_timeout: Duration::from_millis(100),
            enable_batching: true,
            kafka_properties: HashMap::new(),
        }
    }
}

/// Builder for ConsumerConfig
pub struct ConsumerConfigBuilder {
    config: ConsumerConfig,
}

impl ConsumerConfigBuilder {
    /// Create a new consumer config builder
    pub fn new() -> Self {
        Self {
            config: ConsumerConfig::default(),
        }
    }

    /// Set the broker addresses
    pub fn brokers(mut self, brokers: String) -> Self {
        self.config.brokers = brokers;
        self
    }

    /// Set the consumer group ID
    pub fn group_id(mut self, group_id: String) -> Self {
        self.config.group_id = group_id;
        self
    }

    /// Set the topics to consume
    pub fn topics(mut self, topics: Vec<String>) -> Self {
        self.config.topics = topics;
        self
    }

    pub fn session_timeout_ms(mut self, timeout: u32) -> Self {
        self.config.session_timeout_ms = timeout;
        self
    }

    pub fn enable_auto_commit(mut self, enable: bool) -> Self {
        self.config.enable_auto_commit = enable;
        self
    }

    /// Set the batch size for offset commits
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    /// Set the retry policy
    pub fn retry_policy(mut self, policy: crate::consumer::retry::RetryPolicy) -> Self {
        self.config.retry_policy = policy;
        self
    }

    /// Set the dead letter queue topic
    pub fn dlq_topic(mut self, topic: String) -> Self {
        self.config.dlq_topic = Some(topic);
        self
    }

    /// Set the number of retries before sending to DLQ
    pub fn dlq_after_retries(mut self, retries: u32) -> Self {
        self.config.dlq_after_retries = retries;
        self
    }

    /// Set the channel buffer size
    pub fn channel_buffer_size(mut self, size: usize) -> Self {
        self.config.channel_buffer_size = size;
        self
    }

    /// Set the maximum number of inflight messages
    pub fn max_inflight_messages(mut self, max: usize) -> Self {
        self.config.max_inflight_messages = max;
        self
    }

    /// Set the metrics reporting interval
    pub fn metrics_interval(mut self, interval: Duration) -> Self {
        self.config.metrics_interval = interval;
        self
    }

    /// Enable detailed metrics collection
    pub fn enable_detailed_metrics(mut self, enable: bool) -> Self {
        self.config.enable_detailed_metrics = enable;
        self
    }

    /// Set the number of worker threads
    pub fn num_workers(mut self, num_workers: usize) -> Self {
        self.config.num_workers = num_workers;
        self
    }

    /// Set the batch timeout
    pub fn batch_timeout(mut self, timeout: Duration) -> Self {
        self.config.batch_timeout = timeout;
        self
    }

    /// Enable batch processing
    pub fn enable_batching(mut self, enable: bool) -> Self {
        self.config.enable_batching = enable;
        self
    }

    /// Add a custom Kafka property
    pub fn kafka_property(mut self, key: String, value: String) -> Self {
        self.config.kafka_properties.insert(key, value);
        self
    }

    /// Build the consumer configuration
    pub fn build(self) -> ConsumerConfig {
        self.config
    }
}

impl Default for ConsumerConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsumerConfig {
    /// Create a new consumer config builder
    pub fn builder() -> ConsumerConfigBuilder {
        ConsumerConfigBuilder::new()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.brokers.is_empty() {
            return Err("Brokers cannot be empty".to_string());
        }

        if self.group_id.is_empty() {
            return Err("Group ID cannot be empty".to_string());
        }

        if self.topics.is_empty() {
            return Err("Topics cannot be empty".to_string());
        }

        if self.batch_size == 0 {
            return Err("Batch size must be greater than 0".to_string());
        }

        if self.dlq_after_retries > self.retry_policy.max_retries {
            return Err("DLQ retry threshold cannot exceed max retries".to_string());
        }

        if self.pause_threshold <= self.resume_threshold {
            return Err("Pause threshold must be greater than resume threshold".to_string());
        }

        if self.pause_threshold > 1.0 || self.resume_threshold > 1.0 {
            return Err("Thresholds must be between 0 and 1".to_string());
        }

        if self.num_workers == 0 {
            return Err("Number of workers must be greater than 0".to_string());
        }

        if self.batch_timeout.is_zero() {
            return Err("Batch timeout must be greater than 0".to_string());
        }

        Ok(())
    }
}

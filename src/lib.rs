//! High-performance Rust implementation of Sigma rule engine
//!
//! This library provides a complete implementation of the Sigma rule specification
//! with support for real-time event processing, Redpanda integration, and
//! comprehensive performance optimizations.
//!
//! # Example
//!
//! ```no_run
//! use sigma_rs::{DynamicEvent, rule};
//! use serde_json::json;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Parse a Sigma rule
//! let rule = rule::rule_from_yaml(include_bytes!("../../../examples/rules/process_creation.yml"))?;
//!
//! // Create an event
//! let event = DynamicEvent::new(json!({
//!     "EventID": 1,
//!     "CommandLine": "powershell.exe -Command Get-Process"
//! }));
//!
//! // Build the detection tree
//! let tree = sigma_rs::tree::Tree::from_rule(&rule).await?;
//!
//! // Check if the event matches
//! let matches = tree.matches(&event).await?;
//! tracing::error!("Event matches: {}", matches.matched);
//! # Ok(())
//! # }
//! ```
//!
//! # Redpanda Integration
//!
//! ```no_run
//! use sigma_rs::{SigmaEngineBuilder, KafkaConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let kafka_config = KafkaConfig {
//!     brokers: "localhost:9092".to_string(),
//!     group_id: "sigma-processor".to_string(),
//!     topics: vec!["security-events".to_string()],
//!     ..Default::default()
//! };
//!
//! let engine = SigmaEngineBuilder::new()
//!     .add_rule_dir("/path/to/rules")
//!     .with_kafka(kafka_config)
//!     .build()
//!     .await?;
//!
//! engine.run().await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]
#![warn(clippy::all)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::module_inception)]

// Re-export commonly used items
pub use ast::{Branch, MatchResult};
pub use error::{Result, SigmaError};
pub use event::{DynamicEvent, Event, Keyworder, Selector, Value};
pub use ruleset::{ConcurrentRuleSet, RuleMatch, RuleSet, RuleSetResult};

/// Event abstractions and implementations
pub mod event;

/// AST nodes and matching engine
pub mod ast;

/// Error types
pub mod error;

/// Lexical analysis
pub mod lexer;

/// Parser implementation
pub mod parser;

/// Core event abstractions
pub mod core {
    pub use crate::event::Event;
}

/// Rule definitions and YAML parsing
pub mod rule;

/// Pattern matching implementations
pub mod pattern;

/// AST tree structure
pub mod tree;

/// Result types for matches
pub mod result;

/// RuleSet for managing multiple rules
pub mod ruleset;

/// Core engine implementation
pub mod engine;

pub use engine::SigmaEngine;

/// Service layer with Tokio integration
pub mod service;

/// Consumer implementation for Redpanda/Kafka
pub mod consumer;

/// Aggregation support for Sigma rules  
pub mod aggregation;

/// OpenTelemetry integration for distributed tracing
pub mod telemetry;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the tracing subscriber with default settings
pub fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}

/// Builder for configuring the Sigma engine
#[derive(Debug, Clone)]
pub struct SigmaEngineBuilder {
    /// Rule directories to scan
    pub rule_dirs: Vec<String>,
    /// Whether to fail on rule parse errors
    pub fail_on_parse_error: bool,
    /// Whether to collapse whitespace in patterns
    pub collapse_whitespace: bool,
    /// Number of worker threads
    pub worker_threads: usize,
    /// Redpanda configuration
    pub kafka_config: Option<KafkaConfig>,
}

/// Kafka/Redpanda configuration
#[derive(Debug, Clone)]
pub struct KafkaConfig {
    /// Broker addresses
    pub brokers: String,
    /// Consumer group ID
    pub group_id: String,
    /// Topics to consume from
    pub topics: Vec<String>,
    /// Additional Kafka properties
    pub properties: std::collections::HashMap<String, String>,
    /// Batch size for processing
    pub batch_size: Option<usize>,
    /// Maximum retries for failed messages
    pub max_retries: Option<u32>,
    /// Dead letter queue topic
    pub dlq_topic: Option<String>,
    /// Backpressure buffer size
    pub backpressure_buffer_size: Option<usize>,
    /// Enable detailed metrics
    pub enable_metrics: bool,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            group_id: "sigma-consumer".to_string(),
            topics: vec!["events".to_string()],
            properties: std::collections::HashMap::new(),
            batch_size: Some(1000),
            max_retries: Some(3),
            dlq_topic: None,
            backpressure_buffer_size: Some(10000),
            enable_metrics: true,
        }
    }
}

impl Default for SigmaEngineBuilder {
    fn default() -> Self {
        Self {
            rule_dirs: vec![],
            fail_on_parse_error: false,
            collapse_whitespace: true,
            worker_threads: num_cpus::get(),
            kafka_config: None,
        }
    }
}

impl SigmaEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule directory
    pub fn add_rule_dir(mut self, dir: impl Into<String>) -> Self {
        self.rule_dirs.push(dir.into());
        self
    }

    /// Set whether to fail on parse errors
    pub fn fail_on_parse_error(mut self, fail: bool) -> Self {
        self.fail_on_parse_error = fail;
        self
    }

    /// Set whether to collapse whitespace
    pub fn collapse_whitespace(mut self, collapse: bool) -> Self {
        self.collapse_whitespace = collapse;
        self
    }

    /// Set the number of worker threads
    pub fn worker_threads(mut self, threads: usize) -> Self {
        self.worker_threads = threads;
        self
    }

    /// Configure Kafka/Redpanda integration
    pub fn with_kafka(mut self, config: KafkaConfig) -> Self {
        self.kafka_config = Some(config);
        self
    }

    /// Build the Sigma engine
    pub async fn build(self) -> Result<SigmaEngine> {
        SigmaEngine::new(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = SigmaEngineBuilder::new();
        assert_eq!(builder.rule_dirs.len(), 0);
        assert!(!builder.fail_on_parse_error);
        assert!(builder.collapse_whitespace);
        assert_eq!(builder.worker_threads, num_cpus::get());
        assert!(builder.kafka_config.is_none());
    }

    #[test]
    fn test_builder_configuration() {
        let kafka_config = KafkaConfig {
            brokers: "localhost:9092".to_string(),
            group_id: "sigma-test".to_string(),
            topics: vec!["events".to_string()],
            properties: std::collections::HashMap::new(),
            batch_size: Some(100),
            max_retries: Some(3),
            dlq_topic: Some("dlq-events".to_string()),
            backpressure_buffer_size: Some(1000),
            enable_metrics: true,
        };

        let builder = SigmaEngineBuilder::new()
            .add_rule_dir("/path/to/rules")
            .fail_on_parse_error(true)
            .collapse_whitespace(false)
            .worker_threads(4)
            .with_kafka(kafka_config);

        assert_eq!(builder.rule_dirs, vec!["/path/to/rules"]);
        assert!(builder.fail_on_parse_error);
        assert!(!builder.collapse_whitespace);
        assert_eq!(builder.worker_threads, 4);
        assert!(builder.kafka_config.is_some());
    }
}

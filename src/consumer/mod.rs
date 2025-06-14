//! Redpanda/Kafka consumer implementation for the Sigma engine
//!
//! This module provides a production-ready consumer implementation with:
//! - Robust error handling and recovery
//! - Manual offset management
//! - Dead letter queue support
//! - Backpressure control
//! - Comprehensive metrics
//! - Graceful shutdown
//!
//! # Example
//!
//! ```no_run
//! use sigma_rs::consumer::{ConsumerConfig, create_sigma_consumer, MessageProcessor};
//! use sigma_rs::{SigmaEngine, DynamicEvent};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct SigmaProcessor {
//!     engine: Arc<SigmaEngine>,
//! }
//!
//! #[async_trait]
//! impl MessageProcessor for SigmaProcessor {
//!     async fn process_message(&self, event: DynamicEvent) -> anyhow::Result<()> {
//!         // Process the event
//!         self.engine.process_event(event).await?;
//!         Ok(())
//!     }
//! }
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = ConsumerConfig::builder()
//!     .brokers("localhost:9092".to_string())
//!     .group_id("sigma-consumer".to_string())
//!     .topics(vec!["security-events".to_string()])
//!     .batch_size(100)
//!     .build();
//!
//! let engine = Arc::new(SigmaEngine::new());
//! let processor = Arc::new(SigmaProcessor { engine });
//!
//! let consumer = create_sigma_consumer(processor, config).await?;
//! consumer.run().await?;
//! # Ok(())
//! # }
//! ```

pub mod backpressure;
pub mod config;
pub mod consumer;
pub mod dlq;
pub mod error;
pub mod metrics;
pub mod offset_manager;
pub mod processor;
pub mod retry;
pub mod shutdown;

pub use backpressure::{
    AdaptiveBackpressureConfig, AdaptiveBackpressureController, BackpressureController,
};
pub use config::{ConsumerConfig, ConsumerConfigBuilder};
pub use consumer::RedpandaConsumer;
pub use dlq::{DlqConfig, DlqProducer};
pub use error::{ConsumerError, ConsumerResult};
pub use metrics::ConsumerMetrics;
pub use offset_manager::OffsetManager;
pub use processor::MessageProcessor;
pub use retry::{RetryExecutor, RetryPolicy, RetryResult};
pub use shutdown::{ShutdownCoordinator, ShutdownState};

use crate::DynamicEvent;
use crate::SigmaEngine;
use rdkafka::Message;
use std::sync::Arc;
use tracing::info;

/// Default consumer configuration
pub fn default_config() -> ConsumerConfig {
    ConsumerConfig::builder()
        .brokers("localhost:9092".to_string())
        .group_id("sigma-engine".to_string())
        .topics(vec!["events".to_string()])
        .build()
}

/// Create a consumer for the Sigma engine
pub async fn create_sigma_consumer(
    engine: Arc<SigmaEngine>,
    config: ConsumerConfig,
) -> ConsumerResult<RedpandaConsumer<SigmaMessageProcessor>> {
    info!("Creating Sigma consumer with config: {:?}", config);
    let processor = SigmaMessageProcessor::new(engine);
    RedpandaConsumer::new(config, processor).await
}

/// Message processor implementation for Sigma engine
pub struct SigmaMessageProcessor {
    engine: Arc<SigmaEngine>,
}

impl SigmaMessageProcessor {
    pub fn new(engine: Arc<SigmaEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait::async_trait]
impl MessageProcessor for SigmaMessageProcessor {
    type Error = ConsumerError;

    async fn process(&self, message: &rdkafka::message::OwnedMessage) -> Result<(), Self::Error> {
        // Extract payload
        let payload = message
            .payload()
            .ok_or_else(|| ConsumerError::ParseError("Empty message payload".to_string()))?;

        // Parse JSON
        let json: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|e| ConsumerError::ParseError(format!("JSON parse error: {}", e)))?;

        // Create event and process
        let event = DynamicEvent::new(json);
        self.engine
            .process_event(event)
            .await
            .map_err(|e| ConsumerError::ProcessingError(format!("Engine error: {}", e)))?;

        Ok(())
    }

    async fn on_success(&self, _message: &rdkafka::message::OwnedMessage) {
        // Metrics will be updated here
    }

    async fn on_failure(&self, error: &Self::Error, message: &rdkafka::message::OwnedMessage) {
        tracing::error!(
            "Failed to process message from topic: {}, partition: {}, offset: {}, error: {}",
            message.topic(),
            message.partition(),
            message.offset(),
            error
        );
    }
}

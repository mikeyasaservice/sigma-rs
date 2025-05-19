//! Dead Letter Queue (DLQ) handling for failed messages

use crate::consumer::error::{ConsumerError, ConsumerResult};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::message::{OwnedMessage, Headers};
use rdkafka::Message;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, warn};
use serde_json::json;

/// DLQ producer for handling failed messages
#[derive(Clone)]
pub struct DlqProducer {
    producer: Arc<FutureProducer>,
    topic: String,
    timeout: Duration,
    add_metadata: bool,
}

impl DlqProducer {
    /// Create a new DLQ producer
    pub fn new(producer: FutureProducer, topic: String) -> Self {
        Self {
            producer: Arc::new(producer),
            topic,
            timeout: Duration::from_secs(30),
            add_metadata: true,
        }
    }
    
    /// Set the send timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    /// Set whether to add metadata headers
    pub fn with_metadata(mut self, add_metadata: bool) -> Self {
        self.add_metadata = add_metadata;
        self
    }
    
    /// Send a message to the DLQ
    pub async fn send_message(
        &self,
        original_message: &OwnedMessage,
        error: &str,
        attempts: u32,
    ) -> ConsumerResult<()> {
        // Create the DLQ record
        let mut record = FutureRecord::to(&self.topic);
        
        // Copy key if present
        if let Some(key) = original_message.key() {
            record = record.key(key);
        }
        
        // Copy payload if present
        if let Some(payload) = original_message.payload() {
            record = record.payload(payload);
        }
        
        // Add metadata headers if enabled
        if self.add_metadata {
            let headers = self.create_dlq_headers(original_message, error, attempts)?;
            record = record.headers(headers);
        }
        
        // Send to DLQ
        match self.producer.send(record, self.timeout).await {
            Ok((partition, offset)) => {
                debug!(
                    "Message sent to DLQ topic: {}, partition: {}, offset: {}",
                    self.topic, partition, offset
                );
                Ok(())
            }
            Err((e, _)) => {
                error!("Failed to send message to DLQ: {}", e);
                Err(ConsumerError::DlqError(format!("DLQ send failed: {}", e)))
            }
        }
    }
    
    /// Create DLQ headers with metadata
    fn create_dlq_headers(
        &self,
        original_message: &OwnedMessage,
        error: &str,
        attempts: u32,
    ) -> ConsumerResult<rdkafka::message::OwnedHeaders> {
        let mut headers = rdkafka::message::OwnedHeaders::new();
        
        // Add original topic and partition
        headers = headers.insert(rdkafka::message::Header {
            key: "dlq.original.topic",
            value: Some(original_message.topic().as_bytes()),
        });
        
        headers = headers.insert(rdkafka::message::Header {
            key: "dlq.original.partition",
            value: Some(original_message.partition().to_string().as_bytes()),
        });
        
        headers = headers.insert(rdkafka::message::Header {
            key: "dlq.original.offset",
            value: Some(original_message.offset().to_string().as_bytes()),
        });
        
        // Add error information
        headers = headers.insert(rdkafka::message::Header {
            key: "dlq.error.message",
            value: Some(error.as_bytes()),
        });
        
        headers = headers.insert(rdkafka::message::Header {
            key: "dlq.error.attempts",
            value: Some(attempts.to_string().as_bytes()),
        });
        
        // Add timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ConsumerError::DlqError(format!("Time error: {}", e)))?;
        
        headers = headers.insert(rdkafka::message::Header {
            key: "dlq.timestamp",
            value: Some(timestamp.as_secs().to_string().as_bytes()),
        });
        
        // Copy original headers if present
        if let Some(original_headers) = original_message.headers() {
            for header in original_headers.iter() {
                if !header.key.starts_with("dlq.") {
                    let key = format!("dlq.original.header.{}", header.key);
                    headers = headers.insert(rdkafka::message::Header {
                        key: &key,
                        value: header.value,
                    });
                }
            }
        }
        
        Ok(headers)
    }
    
    /// Send a message with a JSON error payload
    pub async fn send_with_error_payload(
        &self,
        original_message: &OwnedMessage,
        error: &str,
        attempts: u32,
        additional_metadata: Option<serde_json::Value>,
    ) -> ConsumerResult<()> {
        // Create error payload
        let error_payload = json!({
            "error": error,
            "attempts": attempts,
            "timestamp": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            "original": {
                "topic": original_message.topic(),
                "partition": original_message.partition(),
                "offset": original_message.offset(),
                "timestamp": original_message.timestamp().to_millis(),
            },
            "metadata": additional_metadata,
            "original_payload": original_message.payload()
                .and_then(|p| String::from_utf8(p.to_vec()).ok()),
        });
        
        let payload_bytes = serde_json::to_vec(&error_payload)
            .map_err(|e| ConsumerError::DlqError(format!("JSON serialization error: {}", e)))?;
        
        // Create the DLQ record
        let mut record = FutureRecord::to(&self.topic)
            .payload(&payload_bytes);
        
        // Use original key if present
        if let Some(key) = original_message.key() {
            record = record.key(key);
        }
        
        // Add metadata headers
        if self.add_metadata {
            let headers = self.create_dlq_headers(original_message, error, attempts)?;
            record = record.headers(headers);
        }
        
        // Send to DLQ
        match self.producer.send(record, self.timeout).await {
            Ok((partition, offset)) => {
                debug!(
                    "Message with error payload sent to DLQ topic: {}, partition: {}, offset: {}",
                    self.topic, partition, offset
                );
                Ok(())
            }
            Err((e, _)) => {
                error!("Failed to send message to DLQ: {}", e);
                Err(ConsumerError::DlqError(format!("DLQ send failed: {}", e)))
            }
        }
    }
}

/// DLQ configuration
#[derive(Debug, Clone)]
pub struct DlqConfig {
    /// DLQ topic name
    pub topic: String,
    /// Whether to add metadata headers
    pub add_metadata: bool,
    /// Send timeout
    pub timeout: Duration,
    /// Whether to create JSON error payloads
    pub json_payload: bool,
}

impl Default for DlqConfig {
    fn default() -> Self {
        Self {
            topic: String::new(),
            add_metadata: true,
            timeout: Duration::from_secs(30),
            json_payload: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dlq_config_default() {
        let config = DlqConfig::default();
        assert!(config.add_metadata);
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(!config.json_payload);
    }
}
use super::SimpleEvent;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

/// Builder for creating test events
pub struct EventBuilder {
    fields: Map<String, Value>,
    timestamp: Option<DateTime<Utc>>,
}

impl EventBuilder {
    /// Create a new EventBuilder
    pub fn new() -> Self {
        Self {
            fields: Map::new(),
            timestamp: None,
        }
    }

    /// Add a field to the event
    pub fn field<K: Into<String>, V: Into<Value>>(mut self, key: K, value: V) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    /// Set the timestamp for the event
    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Build the SimpleEvent
    pub fn build(self) -> SimpleEvent {
        let mut event = SimpleEvent::new(self.fields);
        if let Some(ts) = self.timestamp {
            event.set_timestamp(ts);
        }
        event
    }
}

impl Default for EventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

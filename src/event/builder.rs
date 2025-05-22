use super::SimpleEvent;
use serde_json::{Value, Map};
use chrono::{DateTime, Utc};

/// Builder for creating test events
pub struct EventBuilder {
    fields: Map<String, Value>,
    timestamp: Option<DateTime<Utc>>,
}

impl EventBuilder {
    pub fn new() -> Self {
        Self {
            fields: Map::new(),
            timestamp: None,
        }
    }
    
    pub fn field<K: Into<String>, V: Into<Value>>(mut self, key: K, value: V) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
    
    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
    
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
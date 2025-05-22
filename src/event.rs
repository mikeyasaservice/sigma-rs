/// Core traits for event processing in the Sigma rule engine
use serde::{Serialize, Deserialize};
use anyhow::Result;

// Export EventBuilder for tests
pub use builder::EventBuilder;

/// Trait for events that can provide keyword fields for matching
pub trait Keyworder {
    /// Returns list of fields that are relevant for keyword matching
    /// Returns (fields, applicable) where applicable indicates if this rule type applies
    fn keywords(&self) -> (Vec<String>, bool);
}

/// Trait for events that support key-value selection
pub trait Selector {
    /// Select a value by key from the event
    /// Returns (value, found) where found indicates if the key exists
    fn select(&self, key: &str) -> (Option<Value>, bool);
}

/// Combined event trait that implements both keyword and selection matching
pub trait Event: Keyworder + Selector + Send + Sync {
    /// Get event ID for tracing
    fn id(&self) -> &str;
    
    /// Get event timestamp
    fn timestamp(&self) -> i64;
}

/// Value type that can be returned from selection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum Value {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// Array of values
    Array(Vec<Value>),
    /// Object mapping keys to values
    Object(std::collections::HashMap<String, Value>),
    /// Null value
    #[default]
    Null,
}

/// Module with event builder for testing
pub mod builder;

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;

/// Simple event implementation for testing
#[derive(Debug, Clone)]
pub struct SimpleEvent {
    fields: serde_json::Map<String, JsonValue>,
    timestamp: DateTime<Utc>,
}

impl SimpleEvent {
    /// Create a new SimpleEvent with the given fields
    pub fn new(fields: serde_json::Map<String, JsonValue>) -> Self {
        Self {
            fields,
            timestamp: Utc::now(),
        }
    }
    
    /// Set the timestamp for this event
    pub fn set_timestamp(&mut self, timestamp: DateTime<Utc>) {
        self.timestamp = timestamp;
    }
    
    /// Get a field value by key
    pub fn get_field(&self, key: &str) -> Option<&JsonValue> {
        self.fields.get(key)
    }
}

impl Keyworder for SimpleEvent {
    fn keywords(&self) -> (Vec<String>, bool) {
        let keywords = self.fields.values()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_owned())
            .collect();
        (keywords, true)
    }
}

impl Selector for SimpleEvent {
    fn select(&self, key: &str) -> (Option<Value>, bool) {
        match self.fields.get(key) {
            Some(json_val) => {
                // TODO: Optimize string cloning - consider using Cow<str> or Arc<str>
                let value = match json_val {
                    JsonValue::String(s) => Value::String(s.clone()),
                    JsonValue::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Value::Integer(i)
                        } else if let Some(f) = n.as_f64() {
                            Value::Float(f)
                        } else {
                            Value::Float(n.as_f64().unwrap_or(0.0))
                        }
                    }
                    JsonValue::Bool(b) => Value::Boolean(*b),
                    JsonValue::Array(arr) => {
                        Value::Array(arr.iter().map(|v| {
                            // Simplified conversion
                            match v {
                                JsonValue::String(s) => Value::String(s.clone()),
                                JsonValue::Number(n) => Value::Float(n.as_f64().unwrap_or(0.0)),
                                JsonValue::Bool(b) => Value::Boolean(*b),
                                _ => Value::Null,
                            }
                        }).collect())
                    }
                    JsonValue::Object(obj) => {
                        let map = obj.iter().map(|(k, v)| {
                            let value = match v {
                                JsonValue::String(s) => Value::String(s.clone()),
                                JsonValue::Number(n) => Value::Float(n.as_f64().unwrap_or(0.0)),
                                JsonValue::Bool(b) => Value::Boolean(*b),
                                _ => Value::Null,
                            };
                            (k.clone(), value)
                        }).collect();
                        Value::Object(map)
                    }
                    JsonValue::Null => Value::Null,
                };
                (Some(value), true)
            }
            None => (None, false)
        }
    }
}

impl Event for SimpleEvent {
    fn id(&self) -> &str {
        self.fields.get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    }
    
    fn timestamp(&self) -> i64 {
        self.timestamp.timestamp()
    }
}


impl Value {
    /// Convert value to string if possible
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    
    /// Convert value to integer if possible
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }
    
    /// Convert value to float if possible
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }
    
    /// Convert value to bool if possible
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

/// Example implementation for a dynamic JSON event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicEvent {
    data: serde_json::Value,
    id: String,
    timestamp: i64,
}

impl DynamicEvent {
    /// Create a new dynamic event with the given data
    pub fn new(data: serde_json::Value) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();
        Self { data, id, timestamp }
    }
}

impl Keyworder for DynamicEvent {
    fn keywords(&self) -> (Vec<String>, bool) {
        // For dynamic events, we might extract from specific fields
        if let Some(message) = self.data.get("message").and_then(|v| v.as_str()) {
            return (vec![message.to_owned()], true);
        }
        
        if let Some(alert) = self.data.get("alert").and_then(|v| v.get("signature")).and_then(|v| v.as_str()) {
            return (vec![alert.to_owned()], true);
        }
        
        (vec![], false)
    }
}

impl Selector for DynamicEvent {
    fn select(&self, key: &str) -> (Option<Value>, bool) {
        // Navigate nested keys using dot notation
        let mut current = &self.data;
        
        for part in key.split('.') {
            match current.get(part) {
                Some(value) => current = value,
                None => return (None, false),
            }
        }
        
        // Convert serde_json::Value to our Value type
        let value = match current {
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float(f)
                } else {
                    Value::Null
                }
            },
            serde_json::Value::Bool(b) => Value::Boolean(*b),
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Array(arr) => {
                let values: Vec<Value> = arr.iter()
                    .map(Self::json_to_value)
                    .collect();
                Value::Array(values)
            },
            serde_json::Value::Object(obj) => {
                let map: std::collections::HashMap<String, Value> = obj.iter()
                    .map(|(k, v)| (k.clone(), Self::json_to_value(v)))
                    .collect();
                Value::Object(map)
            },
        };
        
        (Some(value), true)
    }
}

impl Event for DynamicEvent {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

impl DynamicEvent {
    fn json_to_value(json: &serde_json::Value) -> Value {
        match json {
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float(f)
                } else {
                    Value::Null
                }
            },
            serde_json::Value::Bool(b) => Value::Boolean(*b),
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Array(arr) => {
                let values: Vec<Value> = arr.iter()
                    .map(Self::json_to_value)
                    .collect();
                Value::Array(values)
            },
            serde_json::Value::Object(obj) => {
                let map: std::collections::HashMap<String, Value> = obj.iter()
                    .map(|(k, v)| (k.clone(), Self::json_to_value(v)))
                    .collect();
                Value::Object(map)
            },
        }
    }
}

/// Event trait for async processing
pub trait AsyncEvent: Event {
    /// Async version of keywords for events that need async processing
    fn keywords_async(&self) -> impl std::future::Future<Output = Result<(Vec<String>, bool)>> + Send {
        async { Ok(self.keywords()) }
    }
    
    /// Async version of select for events that need async processing
    fn select_async(&self, key: &str) -> impl std::future::Future<Output = Result<(Option<Value>, bool)>> + Send {
        async { Ok(self.select(key)) }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dynamic_event_selector() {
        let data = serde_json::json!({
            "message": "test message",
            "nested": {
                "field": "value"
            }
        });
        
        let event = DynamicEvent::new(data);
        
        // Test simple field access
        let (value, found) = event.select("message");
        assert!(found);
        assert_eq!(value.unwrap().as_str(), Some("test message"));
        
        // Test nested field access
        let (value, found) = event.select("nested.field");
        assert!(found);
        assert_eq!(value.unwrap().as_str(), Some("value"));
        
        // Test missing field
        let (_, found) = event.select("missing");
        assert!(!found);
    }
    
    #[test]
    fn test_dynamic_event_keywords() {
        let data = serde_json::json!({
            "message": "test keyword",
            "other": "field"
        });
        
        let event = DynamicEvent::new(data);
        let (keywords, applicable) = event.keywords();
        assert!(applicable);
        assert_eq!(keywords, vec!["test keyword"]);
    }
}
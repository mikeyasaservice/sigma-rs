/// Core traits for event processing in the Sigma rule engine
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use anyhow::Result;

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<Value>),
    Object(std::collections::HashMap<String, Value>),
    Null,
}

/// Module with event adapter for AST
pub mod adapter;

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
            return (vec![message.to_string()], true);
        }
        
        if let Some(alert) = self.data.get("alert").and_then(|v| v.get("signature")).and_then(|v| v.as_str()) {
            return (vec![alert.to_string()], true);
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
                    .map(|v| Self::json_to_value(v))
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
                    .map(|v| Self::json_to_value(v))
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
#[async_trait]
pub trait AsyncEvent: Event {
    /// Async version of keywords for events that need async processing
    async fn keywords_async(&self) -> Result<(Vec<String>, bool)> {
        Ok(self.keywords())
    }
    
    /// Async version of select for events that need async processing
    async fn select_async(&self, key: &str) -> Result<(Option<Value>, bool)> {
        Ok(self.select(key))
    }
}

/// Event builder for creating events from various sources
pub struct EventBuilder {
    data: serde_json::Value,
}

impl EventBuilder {
    pub fn new() -> Self {
        Self {
            data: serde_json::Value::Object(serde_json::Map::new()),
        }
    }
    
    pub fn with_field(mut self, key: &str, value: serde_json::Value) -> Self {
        if let serde_json::Value::Object(ref mut map) = self.data {
            map.insert(key.to_string(), value);
        }
        self
    }
    
    pub fn build(self) -> DynamicEvent {
        DynamicEvent::new(self.data)
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
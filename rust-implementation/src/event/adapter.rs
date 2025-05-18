use crate::ast;
use crate::event::{Event, Value, Keyworder, Selector};
use serde_json;

/// Adapter to bridge our Event trait to AST's Event trait
pub struct AstEventAdapter<'a> {
    event: &'a dyn Event,
}

impl<'a> AstEventAdapter<'a> {
    pub fn new(event: &'a dyn Event) -> Self {
        Self { event }
    }
}

impl<'a> ast::Event for AstEventAdapter<'a> {
    fn keywords(&self) -> Vec<String> {
        let (keywords, _) = self.event.keywords();
        keywords
    }
    
    fn select(&self, key: &str) -> Option<serde_json::Value> {
        let (value_opt, _) = self.event.select(key);
        value_opt.map(|v| value_to_json(v))
    }
}

fn value_to_json(value: Value) -> serde_json::Value {
    match value {
        Value::String(s) => serde_json::Value::String(s),
        Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(i)),
        Value::Float(f) => serde_json::Value::Number(
            serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0))
        ),
        Value::Boolean(b) => serde_json::Value::Bool(b),
        Value::Array(arr) => serde_json::Value::Array(
            arr.into_iter().map(value_to_json).collect()
        ),
        Value::Object(obj) => serde_json::Value::Object(
            obj.into_iter()
                .map(|(k, v)| (k, value_to_json(v)))
                .collect()
        ),
        Value::Null => serde_json::Value::Null,
    }
}

/// Simple event implementation for tests
#[derive(Debug)]
pub struct SimpleEvent {
    data: serde_json::Value,
}

impl SimpleEvent {
    pub fn new(data: serde_json::Value) -> Self {
        Self { data }
    }
}

impl ast::Event for SimpleEvent {
    fn keywords(&self) -> Vec<String> {
        // Simple implementation: extract all string values
        let mut keywords = Vec::new();
        extract_keywords(&self.data, &mut keywords);
        keywords
    }
    
    fn select(&self, key: &str) -> Option<serde_json::Value> {
        self.data.get(key).cloned()
    }
}

fn extract_keywords(value: &serde_json::Value, keywords: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => keywords.push(s.clone()),
        serde_json::Value::Array(arr) => {
            for v in arr {
                extract_keywords(v, keywords);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, v) in obj {
                extract_keywords(v, keywords);
            }
        }
        _ => {}
    }
}
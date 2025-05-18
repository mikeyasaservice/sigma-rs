use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Detection represents the detection field in sigma rule
/// Contains condition expression and identifier fields for building AST
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Detection(HashMap<String, Value>);

impl Detection {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Extract all fields except the condition
    pub fn extract(&self) -> HashMap<String, Value> {
        self.0
            .iter()
            .filter(|(k, _)| k.as_str() != "condition")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get a field value by key
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// Get the condition expression
    pub fn condition(&self) -> Option<&str> {
        self.get("condition").and_then(|v| v.as_str())
    }

    /// Insert a field
    pub fn insert(&mut self, key: String, value: Value) {
        self.0.insert(key, value);
    }

    /// Check if a field exists
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Iterate over all fields
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.0.iter()
    }

    /// Get the internal HashMap
    pub fn inner(&self) -> &HashMap<String, Value> {
        &self.0
    }

    /// Convert into the internal HashMap
    pub fn into_inner(self) -> HashMap<String, Value> {
        self.0
    }
}

impl From<HashMap<String, Value>> for Detection {
    fn from(map: HashMap<String, Value>) -> Self {
        Self(map)
    }
}

impl std::ops::Deref for Detection {
    type Target = HashMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Detection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

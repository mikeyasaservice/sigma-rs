use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Detection represents the detection field in sigma rule
/// contains condition expression and identifier fields for building AST
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct Detection(pub HashMap<String, Value>);

impl Detection {
    /// Create a new empty Detection
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    
    /// Get the condition string if present
    pub fn condition(&self) -> Option<&str> {
        self.0.get("condition").and_then(|v| v.as_str())
    }
    
    /// Extract all non-condition keys and their values
    pub fn extract(&self) -> HashMap<String, Value> {
        self.0
            .iter()
            .filter(|(k, _)| k.as_str() != "condition")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
    
    /// Get a specific detection identifier by key
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
    
    /// Check if detection has a specific key
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    
    /// Get the number of detection rules (excluding condition)
    pub fn rule_count(&self) -> usize {
        self.0.len() - if self.condition().is_some() { 1 } else { 0 }
    }
    
    /// Insert a new detection rule
    pub fn insert(&mut self, key: String, value: Value) {
        self.0.insert(key, value);
    }
    
    /// Get an iterator over the detection rules (excluding condition)
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.0.iter().filter(|(k, _)| k.as_str() != "condition")
    }
}

impl From<HashMap<String, Value>> for Detection {
    fn from(map: HashMap<String, Value>) -> Self {
        Self(map)
    }
}

impl AsRef<HashMap<String, Value>> for Detection {
    fn as_ref(&self) -> &HashMap<String, Value> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_detection_condition() {
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), json!("selection1 and selection2"));
        detection.insert("selection1".to_string(), json!({"EventID": 1}));
        detection.insert("selection2".to_string(), json!({"Image": "*\\cmd.exe"}));
        
        assert_eq!(detection.condition(), Some("selection1 and selection2"));
        assert_eq!(detection.rule_count(), 2);
    }
    
    #[test]
    fn test_detection_extract() {
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), json!("all of selection*"));
        detection.insert("selection1".to_string(), json!({"EventID": 1}));
        detection.insert("selection2".to_string(), json!({"Image": "*\\cmd.exe"}));
        
        let extracted = detection.extract();
        assert_eq!(extracted.len(), 2);
        assert!(extracted.contains_key("selection1"));
        assert!(extracted.contains_key("selection2"));
        assert!(!extracted.contains_key("condition"));
    }
    
    #[test]
    fn test_detection_deserialize() {
        let yaml = r#"
condition: selection
selection:
  EventID: 1
  Image|endswith: '\cmd.exe'
        "#;
        
        let detection: Detection = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(detection.condition(), Some("selection"));
        assert!(detection.contains_key("selection"));
    }
}
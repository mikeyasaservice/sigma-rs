use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod detection;
pub mod logsource;
pub mod tags;

pub use detection::Detection;
pub use logsource::Logsource;
pub use tags::Tags;

/// Rule defines raw rule conforming to sigma rule specification
/// https://github.com/Neo23x0/sigma/wiki/Specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    #[serde(default)]
    pub falsepositives: Vec<String>,
    
    #[serde(default)]
    pub fields: Vec<String>,
    
    pub id: String,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    
    pub title: String,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    
    #[serde(default)]
    pub references: Vec<String>,
    
    #[serde(default)]
    pub logsource: Logsource,
    
    pub detection: Detection,
    
    #[serde(default)]
    pub tags: Vec<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
}

impl Rule {
    /// Check if the rule contains all provided tags
    pub fn has_tags(&self, tags: &[String]) -> bool {
        Tags::from(self.tags.clone()).has_all(tags)
    }
}

/// RuleHandle is a meta object containing all fields from raw yaml, but is enhanced to also
/// hold debugging info from the tool, such as source file path, etc
#[derive(Debug, Clone)]
pub struct RuleHandle {
    pub rule: Rule,
    pub path: PathBuf,
    pub multipart: bool,
    pub no_collapse_ws: bool,
}

impl RuleHandle {
    /// Create a new RuleHandle from a Rule and metadata
    pub fn new(rule: Rule, path: PathBuf) -> Self {
        Self {
            rule,
            path,
            multipart: false,
            no_collapse_ws: false,
        }
    }
    
    /// Set whether this is a multipart rule
    pub fn with_multipart(mut self, multipart: bool) -> Self {
        self.multipart = multipart;
        self
    }
    
    /// Set whether to disable whitespace collapsing
    pub fn with_no_collapse_ws(mut self, no_collapse_ws: bool) -> Self {
        self.no_collapse_ws = no_collapse_ws;
        self
    }
}

/// Parse a Rule from YAML data
pub fn rule_from_yaml(data: &[u8]) -> Result<Rule, serde_yaml::Error> {
    serde_yaml::from_slice(data)
}

/// Check if rule data is multipart (contains document separator not at start)
pub fn is_multipart(data: &[u8]) -> bool {
    // Check if data starts with "---"
    let starts_with_separator = data.starts_with(b"---");
    
    // Check if data contains "---" anywhere
    let contains_separator = data
        .windows(3)
        .any(|window| window == b"---");
    
    // Multipart if it contains separator but doesn't start with it
    !starts_with_separator && contains_separator
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rule_from_yaml() {
        let yaml = r#"
title: Test Rule
id: 12345678-1234-1234-1234-123456789012
description: A test rule
author: Test Author
date: 2024/01/01
status: experimental
level: medium
references:
  - https://example.com
tags:
  - attack.discovery
  - attack.t1069.001
logsource:
  product: windows
  category: process_creation
detection:
  selection:
    EventID: 1
  condition: selection
falsepositives:
  - Unknown
        "#;
        
        let rule = rule_from_yaml(yaml.as_bytes()).unwrap();
        assert_eq!(rule.title, "Test Rule");
        assert_eq!(rule.id, "12345678-1234-1234-1234-123456789012");
        assert_eq!(rule.author, Some("Test Author".to_string()));
        assert_eq!(rule.level, Some("medium".to_string()));
        assert_eq!(rule.tags.len(), 2);
        assert!(rule.has_tags(&["attack.discovery".to_string()]));
    }
    
    #[test]
    fn test_multipart_detection() {
        let single_doc = b"---\ntitle: Test";
        let multi_doc = b"title: Test\n---\ntitle: Test2";
        let no_separator = b"title: Test";
        
        assert!(!is_multipart(single_doc));
        assert!(is_multipart(multi_doc));
        assert!(!is_multipart(no_separator));
    }
}
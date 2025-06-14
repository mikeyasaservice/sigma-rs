//! Sigma rule parsing and representation
//!
//! This module provides structures and functions for parsing Sigma rules
//! from YAML format and converting them into an internal representation.
//!
//! # Example
//!
//! ```
//! use sigma_rs::rule::{rule_from_yaml, Rule};
//!
//! # fn example() -> anyhow::Result<()> {
//! let yaml_content = r#"
//! title: Suspicious Process Creation
//! id: 12345678-1234-1234-1234-123456789abc
//! status: stable
//! description: Detects suspicious process creation
//! detection:
//!   selection:
//!     EventID: 1
//!     CommandLine|contains: 'powershell'
//!   condition: selection
//! "#;
//!
//! let rule = rule_from_yaml(yaml_content.as_bytes())?;
//! assert_eq!(rule.title, "Suspicious Process Creation");
//! # Ok(())
//! # }
//! ```

use crate::error::{Result, SigmaError};
use regex::Regex;
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
    /// Rule author
    pub author: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Rule description
    pub description: Option<String>,

    #[serde(default)]
    /// Known false positive scenarios
    pub falsepositives: Vec<String>,

    #[serde(default)]
    /// Fields relevant to this rule
    pub fields: Vec<String>,

    /// Unique rule identifier
    pub id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Severity level
    pub level: Option<String>,

    /// Rule title
    pub title: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Rule status (experimental, testing, stable)
    pub status: Option<String>,

    #[serde(default)]
    /// External references
    pub references: Vec<String>,

    #[serde(default)]
    /// Log source configuration
    pub logsource: Logsource,

    /// Detection rules and conditions
    pub detection: Detection,

    #[serde(default)]
    /// Rule tags for categorization
    pub tags: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Creation date
    pub date: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Last modification date
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
    /// The parsed rule
    pub rule: Rule,
    /// Source file path
    pub path: PathBuf,
    /// Whether this is a multipart rule
    pub multipart: bool,
    /// Whether to preserve whitespace in patterns
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

/// Parse a Rule from YAML data with validation
pub fn rule_from_yaml(data: &[u8]) -> Result<Rule> {
    let rule: Rule = serde_yaml::from_slice(data)?;
    validate_rule(&rule)?;
    Ok(rule)
}

/// Validate that a rule meets the minimum requirements
fn validate_rule(rule: &Rule) -> Result<()> {
    // Validate title is not empty
    if rule.title.trim().is_empty() {
        return Err(SigmaError::InvalidRule(
            "Rule title cannot be empty".to_string(),
        ));
    }

    // Validate ID format (should be UUID-like)
    // Format: 8-4-4-4-12 hexadecimal characters
    let id_regex = Regex::new(
        r"^[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}$",
    )
    .expect("Invalid regex pattern");

    if !id_regex.is_match(&rule.id) {
        return Err(SigmaError::InvalidRule(format!(
            "Rule ID '{}' is not a valid UUID format",
            rule.id
        )));
    }

    // Validate detection has a condition
    if rule.detection.condition().is_none() {
        return Err(SigmaError::MissingCondition);
    }

    // Validate that all selections referenced in condition exist
    if let Some(condition) = rule.detection.condition() {
        // Extract identifiers from condition (simple validation)
        // This is a basic check - the full validation happens during parsing
        let selections = rule.detection.extract();

        // Check for basic selection references (not comprehensive, parser does full check)
        for (key, _) in selections.iter() {
            // Basic validation that we have at least one selection
            // The parser will do comprehensive validation of the condition
            if key.is_empty() {
                return Err(SigmaError::InvalidRule(
                    "Detection contains empty selection key".to_string(),
                ));
            }
        }

        // Ensure we have at least one selection when condition is not trivial
        if selections.is_empty() && !matches!(condition.trim(), "true" | "false" | "1" | "0") {
            return Err(SigmaError::InvalidRule(
                "Detection must contain at least one selection".to_string(),
            ));
        }
    }

    Ok(())
}

/// Check if rule data is multipart (contains document separator not at start)
pub fn is_multipart(data: &[u8]) -> bool {
    // Check if data starts with "---"
    let starts_with_separator = data.starts_with(b"---");

    // Check if data contains "---" anywhere
    let contains_separator = data.windows(3).any(|window| window == b"---");

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

        let rule = rule_from_yaml(yaml.as_bytes()).expect("Failed to parse valid test YAML");
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

    #[test]
    fn test_rule_validation_empty_title() {
        let yaml = r#"
title: "   "
id: 12345678-1234-1234-1234-123456789012
detection:
  selection:
    EventID: 1
  condition: selection
        "#;

        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("title cannot be empty"));
    }

    #[test]
    fn test_rule_validation_invalid_id_format() {
        let yaml = r#"
title: Test Rule
id: not-a-valid-uuid
detection:
  selection:
    EventID: 1
  condition: selection
        "#;

        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not a valid UUID format"));
    }

    #[test]
    fn test_rule_validation_missing_condition() {
        let yaml = r#"
title: Test Rule
id: 12345678-1234-1234-1234-123456789012
detection:
  selection:
    EventID: 1
        "#;

        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing condition"));
    }

    #[test]
    fn test_rule_validation_no_selections() {
        let yaml = r#"
title: Test Rule
id: 12345678-1234-1234-1234-123456789012
detection:
  condition: selection
        "#;

        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must contain at least one selection"));
    }

    #[test]
    fn test_rule_validation_valid_edge_cases() {
        // Test with boolean condition (should be valid even without selections)
        let yaml = r#"
title: Always True Rule
id: 12345678-1234-1234-1234-123456789012
detection:
  condition: "true"
        "#;

        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_ok());

        // Test with numeric condition
        let yaml2 = r#"
title: Always False Rule
id: 12345678-1234-1234-1234-123456789012
detection:
  condition: "0"
        "#;

        let result2 = rule_from_yaml(yaml2.as_bytes());
        assert!(result2.is_ok());
    }
}

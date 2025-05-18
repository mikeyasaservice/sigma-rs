use async_trait::async_trait;
use serde::Serialize;
use std::fmt::Debug;

pub mod nodes;
pub use nodes::*;

/// Event trait for matching against AST nodes
pub trait Event: Send + Sync {
    /// Get keywords from the event
    fn keywords(&self) -> Vec<String>;
    
    /// Select a field value by key
    fn select(&self, key: &str) -> Option<serde_json::Value>;
}

/// Result of a match operation
#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult {
    pub matched: bool,
    pub applicable: bool,
}

impl MatchResult {
    pub fn new(matched: bool, applicable: bool) -> Self {
        Self { matched, applicable }
    }

    pub fn matched() -> Self {
        Self {
            matched: true,
            applicable: true,
        }
    }

    pub fn not_matched() -> Self {
        Self {
            matched: false,
            applicable: true,
        }
    }

    pub fn not_applicable() -> Self {
        Self {
            matched: false,
            applicable: false,
        }
    }
}

/// Base trait for all AST nodes
#[async_trait]
pub trait Branch: Debug + Send + Sync {
    /// Match the node against an event
    async fn matches(&self, event: &dyn Event) -> MatchResult;
    
    /// Get a human-readable description of the node
    fn describe(&self) -> String;
}

/// Field rule for matching event fields
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FieldRule {
    pub field: String,
    pub pattern: FieldPattern,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum FieldPattern {
    Exact(String),
    Glob(String),
    Regex(String),
    Keywords(Vec<String>),
}

impl FieldRule {
    pub fn new(field: String, pattern: FieldPattern) -> Self {
        Self { field, pattern }
    }

    pub async fn matches(&self, event: &dyn Event) -> MatchResult {
        let value = match event.select(&self.field) {
            Some(v) => v,
            None => return MatchResult::not_applicable(),
        };

        let matched = match &self.pattern {
            FieldPattern::Exact(s) => {
                value.as_str().map(|v| v == s).unwrap_or(false)
            }
            FieldPattern::Glob(pattern) => {
                // TODO: Implement glob matching
                if let Some(v) = value.as_str() {
                    glob::Pattern::new(pattern)
                        .map(|p| p.matches(v))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            FieldPattern::Regex(pattern) => {
                // TODO: Implement regex matching
                if let Some(v) = value.as_str() {
                    regex::Regex::new(pattern)
                        .map(|r| r.is_match(v))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            FieldPattern::Keywords(keywords) => {
                let event_keywords = event.keywords();
                keywords
                    .iter()
                    .all(|k| event_keywords.contains(k))
            }
        };

        MatchResult::new(matched, true)
    }
}

#[async_trait]
impl Branch for FieldRule {
    async fn matches(&self, event: &dyn Event) -> MatchResult {
        self.matches(event).await
    }

    fn describe(&self) -> String {
        format!("{} matches {:?}", self.field, self.pattern)
    }
}

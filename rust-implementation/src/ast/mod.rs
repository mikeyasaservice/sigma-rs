use async_trait::async_trait;
use serde::{Serialize, Deserialize};
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

use crate::pattern::{StringMatcher, NumMatcher, TextPatternModifier, new_string_matcher, new_num_matcher};
use std::sync::Arc;

/// Field rule for matching event fields
#[derive(Debug, Clone, Serialize)]
pub struct FieldRule {
    pub field: String,
    pub pattern: FieldPattern,
}

/// Field pattern types for matching
#[derive(Debug, Clone)]
pub enum FieldPattern {
    /// String-based pattern matching
    String {
        matcher: Arc<dyn StringMatcher>,
        pattern_desc: String,
    },
    /// Numeric pattern matching
    Numeric {
        matcher: Arc<dyn NumMatcher>,
        pattern_desc: String,
    },
    /// Keyword matching against event keywords
    Keywords(Vec<String>),
}

// Implement Serialize for compatibility
impl serde::Serialize for FieldPattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            FieldPattern::String { pattern_desc, .. } => {
                serializer.serialize_str(pattern_desc)
            }
            FieldPattern::Numeric { pattern_desc, .. } => {
                serializer.serialize_str(pattern_desc)
            }
            FieldPattern::Keywords(keywords) => {
                keywords.serialize(serializer)
            }
        }
    }
}

// Implement PartialEq for compatibility
impl PartialEq for FieldPattern {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FieldPattern::String { pattern_desc: p1, .. }, FieldPattern::String { pattern_desc: p2, .. }) => p1 == p2,
            (FieldPattern::Numeric { pattern_desc: p1, .. }, FieldPattern::Numeric { pattern_desc: p2, .. }) => p1 == p2,
            (FieldPattern::Keywords(k1), FieldPattern::Keywords(k2)) => k1 == k2,
            _ => false,
        }
    }
}

impl FieldRule {
    pub fn new(field: String, pattern: FieldPattern) -> Self {
        Self { field, pattern }
    }
    
    /// Create a string pattern
    pub fn string_pattern(field: String, pattern: String, modifier: TextPatternModifier) -> Result<Self, String> {
        let matcher = new_string_matcher(
            modifier,
            false,  // lowercase
            false,  // all
            false,  // no_collapse_ws
            vec![pattern.clone()],
        )?;
        
        Ok(Self {
            field,
            pattern: FieldPattern::String {
                matcher: Arc::from(matcher),
                pattern_desc: pattern,
            },
        })
    }
    
    /// Create a glob pattern
    pub fn glob_pattern(field: String, pattern: String) -> Result<Self, String> {
        let matcher = new_string_matcher(
            TextPatternModifier::None,
            false,  // lowercase
            false,  // all
            false,  // no_collapse_ws
            vec![pattern.clone()],
        )?;
        
        Ok(Self {
            field,
            pattern: FieldPattern::String {
                matcher: Arc::from(matcher),
                pattern_desc: pattern,
            },
        })
    }

    pub async fn matches(&self, event: &dyn Event) -> MatchResult {
        match &self.pattern {
            FieldPattern::String { matcher, .. } => {
                let value = match event.select(&self.field) {
                    Some(v) => v,
                    None => return MatchResult::not_applicable(),
                };
                
                let value_str = match value.as_str() {
                    Some(s) => s,
                    None => return MatchResult::not_matched(),
                };
                
                MatchResult::new(matcher.string_match(value_str), true)
            }
            FieldPattern::Numeric { matcher, .. } => {
                let value = match event.select(&self.field) {
                    Some(v) => v,
                    None => return MatchResult::not_applicable(),
                };
                
                let num_value = match value.as_i64() {
                    Some(n) => n,
                    None => return MatchResult::not_matched(),
                };
                
                MatchResult::new(matcher.num_match(num_value), true)
            }
            FieldPattern::Keywords(keywords) => {
                let event_keywords = event.keywords();
                let matched = keywords
                    .iter()
                    .all(|k| event_keywords.contains(k));
                MatchResult::new(matched, true)
            }
        }
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

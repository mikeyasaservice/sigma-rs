use async_trait::async_trait;
use std::fmt::Debug;
use crate::pattern::coercion::{coerce_for_string_match, coerce_for_numeric_match};
use crate::event::{Event, Value};

/// AST node implementations
pub mod nodes;
pub use nodes::*;

/// Convert our Value type to serde_json::Value for pattern matching
fn value_to_json(value: Value) -> serde_json::Value {
    match value {
        Value::String(s) => serde_json::Value::String(s.to_string()),
        Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(i)),
        Value::Float(f) => serde_json::Value::Number(
            serde_json::Number::from_f64(f).unwrap_or_else(|| serde_json::Number::from(0))
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

/// Result of a match operation
#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult {
    /// Whether the match was successful
    pub matched: bool,
    /// Whether the rule was applicable
    pub applicable: bool,
}

impl MatchResult {
    /// Create a new match result
    pub fn new(matched: bool, applicable: bool) -> Self {
        Self { matched, applicable }
    }

    /// Create a successful match result
    pub fn matched() -> Self {
        Self {
            matched: true,
            applicable: true,
        }
    }

    /// Create a failed match result
    pub fn not_matched() -> Self {
        Self {
            matched: false,
            applicable: true,
        }
    }

    /// Create a not applicable result
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

use crate::pattern::{StringMatcher, NumMatcher, TextPatternModifier, new_string_matcher};
use std::sync::Arc;

/// Field rule for matching event fields
#[derive(Debug, Clone)]
pub struct FieldRule {
    /// The field name to match against
    pub field: Arc<str>,
    /// The pattern to use for matching
    pub pattern: FieldPattern,
}

/// Field pattern types for matching
#[derive(Debug, Clone)]
pub enum FieldPattern {
    /// String-based pattern matching
    String {
        /// String matcher implementation
        matcher: Arc<dyn StringMatcher>,
        /// Human-readable description of the pattern
        pattern_desc: Arc<str>,
    },
    /// Numeric pattern matching
    Numeric {
        /// Numeric matcher implementation
        matcher: Arc<dyn NumMatcher>,
        /// Human-readable description of the pattern
        pattern_desc: Arc<str>,
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
                serializer.serialize_str(pattern_desc.as_ref())
            }
            FieldPattern::Numeric { pattern_desc, .. } => {
                serializer.serialize_str(pattern_desc.as_ref())
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
    /// Create a new field rule
    pub fn new(field: Arc<str>, pattern: FieldPattern) -> Self {
        Self { field, pattern }
    }
    
    /// Create a string pattern
    pub fn string_pattern(field: Arc<str>, pattern: String, modifier: TextPatternModifier) -> Result<Self, String> {
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
                pattern_desc: Arc::from(pattern),
            },
        })
    }
    
    /// Create a glob pattern
    pub fn glob_pattern(field: Arc<str>, pattern: String) -> Result<Self, String> {
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
                pattern_desc: Arc::from(pattern),
            },
        })
    }

    /// Check if this field rule matches the given event
    pub async fn matches(&self, event: &dyn Event) -> MatchResult {
        match &self.pattern {
            FieldPattern::String { matcher, pattern_desc: _ } => {
                let (value_opt, found) = event.select(self.field.as_ref());
                let value = match value_opt {
                    Some(v) if found => v,
                    _ => return MatchResult::not_applicable(),
                };
                
                // Convert our Value to serde_json::Value for coercion
                let json_value = value_to_json(value);
                let value_str = coerce_for_string_match(&json_value);
                MatchResult::new(matcher.string_match(&value_str), true)
            }
            FieldPattern::Numeric { matcher, .. } => {
                let (value_opt, found) = event.select(self.field.as_ref());
                let value = match value_opt {
                    Some(v) if found => v,
                    _ => return MatchResult::not_applicable(),
                };
                
                // Convert our Value to serde_json::Value for coercion
                let json_value = value_to_json(value);
                let num_value = match coerce_for_numeric_match(&json_value) {
                    Some(n) => n,
                    None => return MatchResult::not_matched(),
                };
                
                MatchResult::new(matcher.num_match(num_value), true)
            }
            FieldPattern::Keywords(keywords) => {
                let (event_keywords, applicable) = event.keywords();
                if !applicable {
                    return MatchResult::not_applicable();
                }
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

// Custom serialization implementation for FieldRule
impl serde::Serialize for FieldRule {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("FieldRule", 2)?;
        state.serialize_field("field", self.field.as_ref())?;
        state.serialize_field("pattern", &self.pattern)?;
        state.end()
    }
}

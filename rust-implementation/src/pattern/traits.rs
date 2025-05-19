//! Core traits for pattern matching

use std::fmt::Debug;

/// Trait for string pattern matchers
pub trait StringMatcher: Debug + Send + Sync {
    /// Match a string value against this pattern
    fn string_match(&self, value: &str) -> bool;
}

/// Trait for numeric pattern matchers
pub trait NumMatcher: Debug + Send + Sync {
    /// Match a numeric value against this pattern
    fn num_match(&self, value: i64) -> bool;
}

/// Result of a pattern match operation
#[derive(Debug, Clone, PartialEq)]
pub struct PatternMatchResult {
    pub matched: bool,
    pub applicable: bool,
}

impl PatternMatchResult {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_match_result() {
        let result = PatternMatchResult::matched();
        assert!(result.matched);
        assert!(result.applicable);

        let result = PatternMatchResult::not_matched();
        assert!(!result.matched);
        assert!(result.applicable);

        let result = PatternMatchResult::not_applicable();
        assert!(!result.matched);
        assert!(!result.applicable);
    }
}
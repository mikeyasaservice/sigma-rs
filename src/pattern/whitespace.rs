//! Whitespace handling for Sigma patterns
//!
//! Implements whitespace collapsing for non-regex patterns as per Sigma specification.

use crate::pattern::security::safe_regex_compile;
use once_cell::sync::Lazy;
use regex::Regex;
use std::borrow::Cow;

/// Global regex for whitespace collapsing (compiled once, reused many times for optimal performance)
static WHITESPACE_COLLAPSE: Lazy<Regex> = Lazy::new(|| {
    // This is a simple whitespace regex that should never fail
    safe_regex_compile(r"\s+").unwrap_or_else(|_| {
        // Fallback to standard regex if safe_regex_compile fails
        Regex::new(r"\s+").expect("Standard whitespace regex should always compile")
    })
});

/// Handle whitespace in a string according to Sigma rules
///
/// If `no_collapse_ws` is false (default), collapses consecutive whitespace
/// characters into a single space. Otherwise returns the string unchanged.
///
/// Returns `Cow<str>` to avoid allocation when string doesn't need modification.
pub fn handle_whitespace(str: &str, no_collapse_ws: bool) -> Cow<'_, str> {
    if no_collapse_ws {
        Cow::Borrowed(str)
    } else {
        // Check if the string actually contains consecutive whitespace that needs collapsing
        if str.chars().any(|c| c.is_whitespace() && c != ' ') || str.contains("  ") {
            WHITESPACE_COLLAPSE.replace_all(str, " ")
        } else {
            Cow::Borrowed(str)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_whitespace() {
        assert_eq!(handle_whitespace("test", false), "test");
        assert_eq!(handle_whitespace("test", true), "test");
    }

    #[test]
    fn test_single_spaces() {
        assert_eq!(handle_whitespace("test string", false), "test string");
        assert_eq!(handle_whitespace("test string", true), "test string");
    }

    #[test]
    fn test_collapse_whitespace() {
        assert_eq!(handle_whitespace("test  string", false), "test string");
        assert_eq!(handle_whitespace("test   string", false), "test string");
        assert_eq!(handle_whitespace("test\t\tstring", false), "test string");
        assert_eq!(handle_whitespace("test\n\nstring", false), "test string");
        assert_eq!(handle_whitespace("test \t \n string", false), "test string");
    }

    #[test]
    fn test_no_collapse_when_disabled() {
        assert_eq!(handle_whitespace("test  string", true), "test  string");
        assert_eq!(handle_whitespace("test\t\tstring", true), "test\t\tstring");
        assert_eq!(handle_whitespace("test\n\nstring", true), "test\n\nstring");
    }

    #[test]
    fn test_leading_trailing_whitespace() {
        assert_eq!(handle_whitespace("  test  ", false), " test ");
        assert_eq!(handle_whitespace("\t\ttest\n\n", false), " test ");
        assert_eq!(handle_whitespace("  test  ", true), "  test  ");
    }

    #[test]
    fn test_cow_optimization() {
        // Test that no allocation occurs when no_collapse_ws is true
        let result = handle_whitespace("test", true);
        assert!(matches!(result, Cow::Borrowed(_)));

        // Test that no allocation occurs when no whitespace needs collapsing
        let result = handle_whitespace("test string", false);
        assert!(matches!(result, Cow::Borrowed(_)));

        // Test that allocation only occurs when necessary
        let result = handle_whitespace("test  string", false);
        assert!(matches!(result, Cow::Owned(_)));
    }
}

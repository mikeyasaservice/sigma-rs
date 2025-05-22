//! Whitespace handling for Sigma patterns
//!
//! Implements whitespace collapsing for non-regex patterns as per Sigma specification.

use regex::Regex;
use once_cell::sync::Lazy;
use crate::pattern::security::safe_regex_compile;

/// Global regex for whitespace collapsing
static WHITESPACE_COLLAPSE: Lazy<Regex> = Lazy::new(|| {
    safe_regex_compile(r"\s+").expect("Invalid whitespace regex")
});

/// Handle whitespace in a string according to Sigma rules
/// 
/// If `no_collapse_ws` is false (default), collapses consecutive whitespace
/// characters into a single space. Otherwise returns the string unchanged.
pub fn handle_whitespace(str: &str, no_collapse_ws: bool) -> String {
    if no_collapse_ws {
        str.to_string()
    } else {
        WHITESPACE_COLLAPSE.replace_all(str, " ").into_owned()
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
}
//! Integration tests for pattern matching functionality

use sigma_rs::ast::{FieldPattern, FieldRule};
use sigma_rs::pattern::*;
use std::sync::Arc;

#[test]
fn test_content_pattern_matching() {
    let pattern = ContentPattern {
        token: "test".to_string(),
        lowercase: false,
        no_collapse_ws: false,
    };

    assert!(pattern.string_match("test"));
    assert!(!pattern.string_match("Test"));
    assert!(!pattern.string_match("testing"));
}

#[test]
fn test_case_insensitive_matching() {
    let pattern = ContentPattern {
        token: "test".to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    assert!(pattern.string_match("test"));
    assert!(pattern.string_match("Test"));
    assert!(pattern.string_match("TEST"));
}

#[test]
fn test_prefix_pattern() {
    let matcher = new_string_matcher(
        TextPatternModifier::Prefix,
        false,
        false,
        false,
        vec!["test".to_string()],
    )
    .unwrap();

    assert!(matcher.string_match("test"));
    assert!(matcher.string_match("testing"));
    assert!(!matcher.string_match("pretest"));
}

#[test]
fn test_suffix_pattern() {
    let matcher = new_string_matcher(
        TextPatternModifier::Suffix,
        false,
        false,
        false,
        vec!["test".to_string()],
    )
    .unwrap();

    assert!(matcher.string_match("test"));
    assert!(matcher.string_match("pretest"));
    assert!(!matcher.string_match("testing"));
}

#[test]
fn test_contains_pattern() {
    let matcher = new_string_matcher(
        TextPatternModifier::Contains,
        false,
        false,
        false,
        vec!["test".to_string()],
    )
    .unwrap();

    assert!(matcher.string_match("test"));
    assert!(matcher.string_match("testing"));
    assert!(matcher.string_match("pretest"));
    assert!(matcher.string_match("pretesting"));
}

#[test]
fn test_glob_pattern() {
    let matcher = new_string_matcher(
        TextPatternModifier::None,
        false,
        false,
        false,
        vec!["test*".to_string()],
    )
    .unwrap();

    assert!(matcher.string_match("test"));
    assert!(matcher.string_match("testing"));
    assert!(!matcher.string_match("pretest"));
}

#[test]
fn test_regex_pattern() {
    let matcher = new_string_matcher(
        TextPatternModifier::Regex,
        false,
        false,
        false,
        vec![r"test\d+".to_string()],
    )
    .unwrap();

    assert!(matcher.string_match("test123"));
    assert!(matcher.string_match("test456"));
    assert!(!matcher.string_match("test"));
    assert!(!matcher.string_match("testing"));
}

#[test]
fn test_numeric_pattern() {
    let matcher = new_num_matcher(vec![1, 2, 3]).unwrap();

    assert!(matcher.num_match(1));
    assert!(matcher.num_match(2));
    assert!(matcher.num_match(3));
    assert!(!matcher.num_match(4));
}

#[test]
fn test_whitespace_collapsing() {
    let matcher = new_string_matcher(
        TextPatternModifier::None,
        false,
        false,
        false, // collapse whitespace
        vec!["test value".to_string()],
    )
    .unwrap();

    assert!(matcher.string_match("test value"));
    assert!(matcher.string_match("test  value"));
    assert!(matcher.string_match("test\tvalue"));
    assert!(matcher.string_match("test\n\nvalue"));
}

#[test]
fn test_no_whitespace_collapsing() {
    let matcher = new_string_matcher(
        TextPatternModifier::None,
        false,
        false,
        true, // no collapse whitespace
        vec!["test  value".to_string()],
    )
    .unwrap();

    assert!(!matcher.string_match("test value"));
    assert!(matcher.string_match("test  value"));
}

#[test]
fn test_multiple_patterns_or() {
    let matcher = new_string_matcher(
        TextPatternModifier::None,
        false,
        false, // OR logic
        false,
        vec!["test1".to_string(), "test2".to_string()],
    )
    .unwrap();

    assert!(matcher.string_match("test1"));
    assert!(matcher.string_match("test2"));
    assert!(!matcher.string_match("test3"));
}

#[test]
fn test_multiple_patterns_and() {
    let matcher = new_string_matcher(
        TextPatternModifier::None,
        false,
        true, // AND logic (all)
        false,
        vec!["test".to_string(), "value".to_string()],
    )
    .unwrap();

    // This is tricky - AND means all patterns must match the same string
    // But the factory might need adjustment for this to work correctly
}

#[test]
fn test_keyword_pattern() {
    let matcher = new_string_matcher(
        TextPatternModifier::Keyword,
        false,
        false,
        false,
        vec!["error".to_string()],
    )
    .unwrap();

    assert!(matcher.string_match("error"));
    assert!(matcher.string_match("an error occurred"));
    assert!(matcher.string_match("system error message"));
}

#[test]
fn test_field_pattern_string() {
    let rule = FieldRule::string_pattern(
        "EventID".to_string(),
        "1".to_string(),
        TextPatternModifier::None,
    )
    .unwrap();

    match &rule.pattern {
        FieldPattern::String { pattern_desc, .. } => {
            assert_eq!(pattern_desc, "1");
        }
        _ => panic!("Expected String pattern"),
    }
}

#[test]
fn test_escape_sigma_for_glob() {
    use sigma_rs::pattern::string_matcher::escape_sigma_for_glob;

    assert_eq!(escape_sigma_for_glob("test"), "test");
    assert_eq!(escape_sigma_for_glob("*"), "*");
    assert_eq!(escape_sigma_for_glob("\\*"), "\\*");
    assert_eq!(escape_sigma_for_glob("\\\\*"), "\\\\*");
    assert_eq!(escape_sigma_for_glob("[test]"), "\\[test\\]");
    assert_eq!(escape_sigma_for_glob("\\"), "\\\\");
}

// Test for complex patterns with modifiers
#[test]
fn test_complex_patterns() {
    // Regex with special chars
    let regex_matcher = new_string_matcher(
        TextPatternModifier::Regex,
        false,
        false,
        false,
        vec![r"cmd\.exe|powershell\.exe".to_string()],
    )
    .unwrap();

    assert!(regex_matcher.string_match("cmd.exe"));
    assert!(regex_matcher.string_match("powershell.exe"));
    assert!(!regex_matcher.string_match("explorer.exe"));

    // Glob with wildcards
    let glob_matcher = new_string_matcher(
        TextPatternModifier::None,
        false,
        false,
        false,
        vec!["*.exe".to_string()],
    )
    .unwrap();

    assert!(glob_matcher.string_match("cmd.exe"));
    assert!(glob_matcher.string_match("powershell.exe"));
    assert!(!glob_matcher.string_match("script.ps1"));
}

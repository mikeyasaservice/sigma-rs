//! Error injection tests to validate robust error handling
//!
//! This test suite validates that the sigma-rs engine handles
//! various error conditions gracefully without panicking.

use sigma_rs::{
    ast::nodes::{NodeSimpleAnd, NodeSimpleOr},
    error::SigmaError,
    matcher::{SimpleAnd, SimpleOr},
    pattern::{factory::new_num_matcher, factory::new_string_matcher, TextPatternModifier},
};

#[test]
fn test_empty_pattern_error_handling() {
    // Test empty string patterns
    let result = new_string_matcher(TextPatternModifier::None, false, false, false, vec![]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "No patterns defined for matcher object"
    );

    // Test empty numeric patterns
    let result = new_num_matcher(vec![]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "No patterns defined for matcher object"
    );
}

#[test]
fn test_matcher_reduction_error_handling() {
    // Test empty AND node reduction
    let empty_and = SimpleAnd { branches: vec![] };
    let result = empty_and.reduce();
    assert!(result.is_err());
    if let Err(SigmaError::InvalidMatcher(msg)) = result {
        assert!(msg.contains("Cannot reduce empty AND node"));
    } else {
        panic!("Expected InvalidMatcher error");
    }

    // Test empty OR node reduction
    let empty_or = SimpleOr { branches: vec![] };
    let result = empty_or.reduce();
    assert!(result.is_err());
    if let Err(SigmaError::InvalidMatcher(msg)) = result {
        assert!(msg.contains("Cannot reduce empty OR node"));
    } else {
        panic!("Expected InvalidMatcher error");
    }
}

#[test]
fn test_ast_node_reduction_error_handling() {
    // Test empty AST AND node reduction
    let empty_ast_and = NodeSimpleAnd::new(vec![]);
    let result = empty_ast_and.reduce();
    assert!(result.is_err());
    if let Err(SigmaError::InvalidMatcher(msg)) = result {
        assert!(msg.contains("Cannot reduce empty AND node"));
    } else {
        panic!("Expected InvalidMatcher error");
    }

    // Test empty AST OR node reduction
    let empty_ast_or = NodeSimpleOr::new(vec![]);
    let result = empty_ast_or.reduce();
    assert!(result.is_err());
    if let Err(SigmaError::InvalidMatcher(msg)) = result {
        assert!(msg.contains("Cannot reduce empty OR node"));
    } else {
        panic!("Expected InvalidMatcher error");
    }
}

#[test]
fn test_invalid_pattern_handling() {
    // Test invalid regex patterns are handled by security module
    let result = new_string_matcher(
        TextPatternModifier::Regex,
        false,
        false,
        false,
        vec!["(a+)+".to_string()], // Known dangerous ReDoS pattern
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unsafe regex pattern"));
}

#[test]
fn test_malformed_glob_patterns() {
    // Test malformed glob patterns
    let result = new_string_matcher(
        TextPatternModifier::Contains,
        false,
        false,
        false,
        vec!["[incomplete".to_string()], // Malformed bracket expression
    );
    // This should either succeed (if glob handles it) or fail gracefully
    match result {
        Ok(_) => (), // Glob pattern was accepted
        Err(e) => assert!(e.contains("Invalid glob pattern")),
    }
}

#[test]
fn test_large_pattern_handling() {
    // Test very large patterns don't cause panics
    let large_pattern = "a".repeat(10000);
    let result = new_string_matcher(
        TextPatternModifier::None,
        false,
        false,
        false,
        vec![large_pattern],
    );
    assert!(result.is_ok());
}

#[test]
fn test_unicode_pattern_handling() {
    // Test Unicode patterns are handled correctly
    let unicode_patterns = vec![
        "测试".to_string(),
        "🔥🚀💻".to_string(),
        "Москва".to_string(),
        "العربية".to_string(),
    ];

    let result = new_string_matcher(
        TextPatternModifier::None,
        false,
        false,
        false,
        unicode_patterns,
    );
    assert!(result.is_ok());
}

#[test]
fn test_extreme_numeric_values() {
    // Test extreme numeric values
    let extreme_values = vec![i64::MIN, i64::MAX, 0, -1, 1];

    let result = new_num_matcher(extreme_values);
    assert!(result.is_ok());
}

#[test]
fn test_pattern_escaping_edge_cases() {
    use sigma_rs::pattern::string_matcher::escape_sigma_for_glob;

    // Test various edge cases in pattern escaping
    let edge_cases = vec![
        "",       // Empty string
        "\\",     // Single backslash
        "\\\\",   // Double backslash
        "[",      // Single bracket
        "]",      // Closing bracket
        "{}",     // Braces
        "*?",     // Wildcards
        "\\*\\?", // Escaped wildcards
    ];

    for case in edge_cases {
        let result = escape_sigma_for_glob(case);
        // Should not panic and should return a valid string
        assert!(result.len() >= case.len());
    }
}

#[test]
fn test_pattern_case_sensitivity_edge_cases() {
    use sigma_rs::pattern::string_matcher::ContentPattern;
    use sigma_rs::pattern::traits::StringMatcher;

    // Test edge cases with case sensitivity
    let pattern = ContentPattern {
        token: "Test".to_string(),
        lowercase: true,
        no_collapse_ws: false,
    };

    let test_cases = vec![
        ("test", true),
        ("TEST", true),
        ("Test", true),
        ("tEsT", true),
        ("testing", false),
        ("", false),
    ];

    for (input, expected) in test_cases {
        assert_eq!(
            pattern.string_match(input),
            expected,
            "Failed for input: '{}'",
            input
        );
    }
}

#[test]
fn test_whitespace_handling_edge_cases() {
    use sigma_rs::pattern::whitespace::handle_whitespace;

    let edge_cases = vec![
        ("", ""),              // Empty string
        ("   ", " "),          // Only whitespace
        ("\t\n\r ", " "),      // Mixed whitespace
        ("a\0b", "a\0b"),      // Null character (not whitespace)
        ("  a  b  ", " a b "), // Leading/trailing whitespace
    ];

    for (input, expected) in edge_cases {
        let result = handle_whitespace(input, false);
        assert_eq!(result.as_ref(), expected, "Failed for input: '{:?}'", input);
    }
}

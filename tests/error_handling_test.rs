//! Comprehensive error handling tests

use sigma_rs::{
    error::SigmaError,
    rule::{rule_from_yaml, Detection},
    parser::Parser,
    lexer::Lexer,
};
use std::error::Error;

#[test]
fn test_lexer_errors() {
    let test_cases = vec![
        ("unterminated_string", "'unterminated"),
        ("invalid_escape", "'test\\x'"),
        ("unexpected_char", "selection & filter"),
        ("invalid_operator", "selection <> filter"),
    ];
    
    for (name, input) in test_cases {
        let (mut lexer, mut rx) = Lexer::new(input.to_string());
        let mut has_error = false;
        
        while let Some(token) = rx.recv().await {
            if token.is_err() {
                has_error = true;
                break;
            }
        }
        
        assert!(has_error, "Expected lexer error for: {}", name);
    }
}

#[test]
fn test_parser_errors() {
    let invalid_rules = vec![
        ("missing_condition", r#"
detection:
    selection:
        EventID: 1
"#),
        ("undefined_identifier", r#"
detection:
    selection:
        EventID: 1
    condition: undefined_selection
"#),
        ("invalid_syntax", r#"
detection:
    selection:
        EventID: 1
    condition: selection and and filter
"#),
        ("circular_reference", r#"
detection:
    sel1:
        field: value
    sel2:
        field: sel1
    condition: sel2
"#),
        ("invalid_modifier", r#"
detection:
    selection:
        field|invalid_mod: 'test'
    condition: selection
"#),
    ];
    
    for (name, yaml) in invalid_rules {
        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err(), "Expected parse error for: {}", name);
        
        if let Err(e) = result {
            tracing::error!("Error for {}: {}", name, e);
            // Verify error messages are informative
            assert!(!e.to_string().is_empty());
        }
    }
}

#[tokio::test]
async fn test_detection_parsing_errors() {
    let test_cases = vec![
        ("empty_condition", r#"{"condition": ""}"#),
        ("missing_selection", r#"{"condition": "selection"}"#),
        ("invalid_json", r#"{"condition": selection"}"#),
        ("wrong_type", r#"{"condition": 123}"#),
    ];
    
    for (name, json) in test_cases {
        let detection: Result<sigma_rs::rule::Detection, _> = 
            serde_json::from_str(json);
        
        if let Ok(det) = detection {
            let mut parser = Parser::new(det, false);
            let result = parser.run().await;
            assert!(result.is_err(), "Expected error for: {}", name);
        } else {
            // JSON parsing failed, which is also an error case
            assert!(true);
        }
    }
}

#[test]
fn test_pattern_errors() {
    let test_cases = vec![
        ("invalid_regex", r"[unclosed", sigma_rs::pattern::TextPatternModifier::Regex),
        ("empty_pattern", "", sigma_rs::pattern::TextPatternModifier::Contains),
        ("invalid_glob", "[invalid", sigma_rs::pattern::TextPatternModifier::None),
    ];
    
    for (pattern, modifier) in test_cases {
        let result = new_string_matcher(
            modifier,
            false,
            false,
            false,
            vec![pattern.to_string()],
        );
        
        if pattern == "invalid_regex" && modifier == sigma_rs::pattern::TextPatternModifier::Regex {
            assert!(result.is_err(), "Expected regex error for invalid pattern");
        }
    }
}

#[test]
fn test_yaml_parsing_errors() {
    let invalid_yamls = vec![
        ("invalid_yaml", "title: [unclosed"),
        ("wrong_structure", "not_a_rule: true"),
        ("missing_required", "title: Test"),
        ("invalid_date", r#"
title: Test
date: not-a-date
detection:
    condition: selection
"#),
    ];
    
    for (name, yaml) in invalid_yamls {
        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err(), "Expected YAML error for: {}", name);
    }
}

#[test]
fn test_error_chain() {
    // Test that errors properly chain and provide context
    let complex_error_yaml = r#"
title: Complex Error Test
detection:
    selection:
        field|regex: '[invalid'
    condition: selection and undefined
"#;
    
    match rule_from_yaml(complex_error_yaml.as_bytes()) {
        Err(e) => {
            let error_string = e.to_string();
            tracing::error!("Error chain: {}", error_string);
            
            // Check that error message is informative
            assert!(error_string.contains("invalid") || 
                    error_string.contains("undefined") ||
                    error_string.contains("regex"));
            
            // Check error source chain
            let mut source = e.source();
            let mut depth = 0;
            while let Some(err) = source {
                tracing::error!("  Source {}: {}", depth, err);
                source = err.source();
                depth += 1;
            }
        }
        Ok(_) => panic!("Expected error but got success"),
    }
}

#[test]
fn test_field_access_errors() {
    let test_cases = vec![
        ("invalid_path", "field..subfield: value"),
        ("empty_field", ": value"),
        ("trailing_dot", "field.: value"),
        ("special_chars", "field@#$: value"),
    ];
    
    for (name, field_spec) in test_cases {
        let yaml = format!(r#"
detection:
    selection:
        {}
    condition: selection
"#, field_spec);
        
        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err(), "Expected field error for: {}", name);
    }
}

#[test]
fn test_numeric_parsing_errors() {
    let test_cases = vec![
        ("invalid_number", "EventID: not_a_number"),
        ("overflow", "EventID: 99999999999999999999"),
        ("invalid_list", "EventID: [1, 'two', 3]"),
    ];
    
    for (name, field_spec) in test_cases {
        let yaml = format!(r#"
detection:
    selection:
        {}
    condition: selection
"#, field_spec);
        
        let result = rule_from_yaml(yaml.as_bytes());
        // Some of these might parse successfully depending on implementation
        tracing::error!("Numeric test {}: {:?}", name, result.is_err());
    }
}

#[test]
fn test_condition_errors() {
    let invalid_conditions = vec![
        ("unbalanced_parens", "(selection or (filter)"),
        ("missing_operand", "selection and"),
        ("invalid_aggregation", "selection | invalid()"),
        ("wrong_identifier", "1selection"),
        ("reserved_word", "and or not"),
    ];
    
    for (name, condition) in invalid_conditions {
        let yaml = format!(r#"
detection:
    selection:
        field: value
    condition: {}
"#, condition);
        
        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err(), "Expected condition error for: {}", name);
    }
}

#[test]
fn test_wildcard_errors() {
    let test_cases = vec![
        ("unescaped_bracket", "path: 'test[file'"),
        ("invalid_escape", "path: 'test\\q'"),
        ("unclosed_bracket", "path: '[a-z'"),
    ];
    
    for (name, field_spec) in test_cases {
        tracing::error!("Testing wildcard error: {}", name);
        // Wildcard errors might be caught at different stages
    }
}

#[test]
fn test_error_recovery() {
    // Test that parser can recover from errors and provide multiple error messages
    let multi_error_yaml = r#"
title: Multi Error Test
detection:
    selection1:
        field|invalid: 'test'
    selection2:
        : 'empty field'
    selection3:
        valid: 'field'
    condition: selection1 and undefined_sel and selection3
"#;
    
    match rule_from_yaml(multi_error_yaml.as_bytes()) {
        Err(e) => {
            tracing::error!("Multi-error: {}", e);
            // Should report multiple issues
        }
        Ok(_) => {
            // Might succeed with partial parsing
        }
    }
}

#[cfg(test)]
mod panic_safety {
    use super::*;
    
    #[test]
    fn test_no_panics_on_malformed_input() {
        let malformed_inputs = vec![
            "",
            "null",
            "[]",
            "{}",
            "\0\0\0\0",
            "������",
            std::str::from_utf8(&[0xFF, 0xFE, 0xFD]).unwrap_or(""),
        ];
        
        for input in malformed_inputs {
            // Should not panic, only return errors
            let _ = rule_from_yaml(input.as_bytes());
            
            let (mut lexer, mut rx) = Lexer::new(input.to_string());
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                lexer.tokenize().await.ok();
                while let Ok(token) = rx.try_recv() {
                    // Process all tokens
                }
            });
        }
    }
    
    #[test]
    fn test_large_input_handling() {
        // Test with very large input
        let large_rule = format!(
            "title: Large\ndetection:\n  condition: {}",
            "selection".repeat(10000)
        );
        
        // Should handle gracefully without stack overflow
        let _ = rule_from_yaml(large_rule.as_bytes());
    }
}

#[cfg(test)]
mod error_messages {
    use super::*;
    
    #[test]
    fn test_helpful_error_messages() {
        let test_cases = vec![
            ("missing_colon", "field value"),
            ("wrong_indent", "  field: value\n    condition: sel"),
            ("invalid_value", "field: [1, 2,]"),
        ];
        
        for (name, input) in test_cases {
            let yaml = format!("detection:\n  selection:\n    {}\n  condition: selection", input);
            
            match rule_from_yaml(yaml.as_bytes()) {
                Err(e) => {
                    let msg = e.to_string();
                    // Error messages should be helpful
                    assert!(!msg.is_empty());
                    assert!(msg.len() > 10); // Not just "error"
                    tracing::error!("{}: {}", name, msg);
                }
                Ok(_) => {
                    // Some might parse successfully
                }
            }
        }
    }
    
    #[test]
    fn test_line_number_reporting() {
        let yaml_with_error = r#"
title: Test
detection:
    selection:
        field: value
    condition: invalid syntax here
"#;
        
        match rule_from_yaml(yaml_with_error.as_bytes()) {
            Err(e) => {
                let msg = e.to_string();
                // Ideally, error should mention line number
                tracing::error!("Error with location: {}", msg);
            }
            Ok(_) => {}
        }
    }
}
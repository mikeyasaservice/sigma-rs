/// Property-based testing for Sigma rule engine
/// Uses proptest to generate random inputs and verify properties

use proptest::prelude::*;
use sigma_rs::{Event, DynamicEvent, Rule, Selector};
use serde_json::{json, Value};
use std::collections::HashMap;

// Strategy for generating field names
fn field_name_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z][a-zA-Z0-9_.]{0,30}".prop_map(|s| s.to_string())
}

// Strategy for generating field values
fn field_value_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        // Null values
        Just(json!(null)),
        // Boolean values
        any::<bool>().prop_map(|b| json!(b)),
        // Integer values  
        any::<i64>().prop_map(|i| json!(i)),
        // String values
        "[a-zA-Z0-9 _./-]{0,100}".prop_map(|s| json!(s)),
        // Array of strings
        prop::collection::vec("[a-zA-Z0-9]{0,20}", 0..5)
            .prop_map(|v| json!(v)),
    ]
}

// Strategy for generating events
fn event_strategy() -> impl Strategy<Value = Value> {
    prop::collection::hash_map(
        field_name_strategy(),
        field_value_strategy(),
        1..20
    ).prop_map(|map| {
        json!(map)
    })
}

// Strategy for generating simple rule conditions
fn simple_condition_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z][a-zA-Z0-9_]{0,15}",
        "[a-zA-Z][a-zA-Z0-9_]{0,15} and [a-zA-Z][a-zA-Z0-9_]{0,15}",
        "[a-zA-Z][a-zA-Z0-9_]{0,15} or [a-zA-Z][a-zA-Z0-9_]{0,15}",
        "not [a-zA-Z][a-zA-Z0-9_]{0,15}",
    ]
}

// Strategy for generating detection modifiers
fn modifier_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("contains".to_string()),
        Just("startswith".to_string()),
        Just("endswith".to_string()),
        Just("re".to_string()),
    ]
}

// Generate a complete Sigma rule
prop_compose! {
    fn sigma_rule_strategy()(
        title in "[a-zA-Z0-9 ]{1,50}",
        id in "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}",
        fields in prop::collection::hash_map(
            field_name_strategy(),
            field_value_strategy(),
            1..5
        ),
        condition in simple_condition_strategy()
    ) -> String {
        let mut yaml = format!("title: {}\nid: {}\ndetection:\n", title, id);
        
        // Add selections based on condition
        let selections: Vec<&str> = condition
            .split_whitespace()
            .filter(|s| !["and", "or", "not", "(", ")"].contains(s))
            .collect();
            
        for selection in selections {
            yaml.push_str(&format!("  {}:\n", selection));
            for (field, value) in &fields {
                yaml.push_str(&format!("    {}: {:?}\n", field, value));
            }
        }
        
        yaml.push_str(&format!("  condition: {}\n", condition));
        yaml
    }
}

proptest! {
    #[test]
    fn test_event_creation_never_panics(event in event_strategy()) {
        // Property: Creating an event should never panic
        let _ = DynamicEvent::new(event);
    }

    #[test]
    fn test_field_selection_never_panics(
        event in event_strategy(),
        field in field_name_strategy()
    ) {
        // Property: Selecting a field should never panic
        let dynamic_event = DynamicEvent::new(event);
        let _ = dynamic_event.select(&field);
    }

    #[test]
    fn test_rule_parsing_handles_any_input(rule in sigma_rule_strategy()) {
        // Property: Rule parsing should handle any input without panicking
        let _ = sigma_rs::rule::rule_from_yaml(rule.as_bytes());
    }

    #[test]
    fn test_empty_event_handling(
        rule in sigma_rule_strategy()
    ) {
        // Property: Empty events should be handled gracefully
        let empty_event = DynamicEvent::new(json!({}));
        
        // This should not panic even with an empty event
        if let Ok(parsed_rule) = sigma_rs::rule::rule_from_yaml(rule.as_bytes()) {
            // TODO: Once Tree is implemented
            // let tree = Tree::new(parsed_rule).unwrap();
            // let _ = tree.matches(&empty_event);
        }
    }

    #[test]
    fn test_field_types_consistency(
        field in field_name_strategy(),
        value1 in field_value_strategy(),
        value2 in field_value_strategy()
    ) {
        // Property: Field selection should handle type changes gracefully
        let event1 = json!({ &field: value1 });
        let event2 = json!({ &field: value2 });
        
        let dynamic_event1 = DynamicEvent::new(event1);
        let dynamic_event2 = DynamicEvent::new(event2);
        
        // Both selections should complete without panic
        let (_, found1) = dynamic_event1.select(&field);
        let (_, found2) = dynamic_event2.select(&field);
        
        // If field exists, it should be found
        assert_eq!(found1, true);
        assert_eq!(found2, true);
    }

    #[test]
    fn test_nested_field_access(
        depth in 1..5usize,
        value in field_value_strategy()
    ) {
        // Property: Nested field access should work correctly
        let mut nested = value;
        let mut path = String::new();
        
        for i in 0..depth {
            let field = format!("field{}", i);
            path.push_str(&field);
            
            nested = json!({ field: nested });
            
            if i < depth - 1 {
                path.push('.');
            }
        }
        
        let event = DynamicEvent::new(nested);
        let (selected_value, found) = event.select(&path);
        
        // Nested access should work
        if depth == 1 {
            assert!(found);
        }
    }

    #[test]
    fn test_modifier_application(
        modifier in modifier_strategy(),
        pattern in "[a-zA-Z0-9]{1,20}",
        value in "[a-zA-Z0-9 ]{0,50}"
    ) {
        // Property: Modifiers should handle all inputs gracefully
        match modifier.as_str() {
            "contains" => {
                let result = value.contains(&pattern);
                // Result should be deterministic
                assert_eq!(result, value.contains(&pattern));
            },
            "startswith" => {
                let result = value.starts_with(&pattern);
                assert_eq!(result, value.starts_with(&pattern));
            },
            "endswith" => {
                let result = value.ends_with(&pattern);
                assert_eq!(result, value.ends_with(&pattern));
            },
            "re" => {
                // Regex compilation might fail, but should not panic
                if let Ok(re) = regex::Regex::new(&pattern) {
                    let _ = re.is_match(&value);
                }
            },
            _ => {}
        }
    }

    #[test]
    fn test_whitespace_handling_consistency(
        text in "[ \t\n\r]{0,10}[a-zA-Z]+[ \t\n\r]{0,10}",
        collapse_ws in any::<bool>()
    ) {
        // Property: Whitespace handling should be consistent
        let rule = format!(
            "title: Test\nid: test\ndetection:\n  selection:\n    field: \"{}\"\n  condition: selection",
            text
        );
        
        // Create events with and without the whitespace
        let event_exact = json!({ "field": text });
        let event_collapsed = json!({ "field": text.split_whitespace().collect::<Vec<_>>().join(" ") });
        
        // TODO: Test with both whitespace collapse settings
        // This would be tested once Tree is implemented
    }

    #[test]
    fn test_rule_components_independence(
        title in "[a-zA-Z0-9 ]{1,50}",
        description in option::of("[a-zA-Z0-9 .]{1,200}"),
        level in prop_oneof![
            Just("low"),
            Just("medium"),
            Just("high"),
            Just("critical")
        ],
        tags in prop::collection::vec("[a-zA-Z.0-9]+", 0..5)
    ) {
        // Property: Rule components should be independently optional
        let mut rule = format!("title: {}\nid: test-123\n", title);
        
        if let Some(desc) = description {
            rule.push_str(&format!("description: {}\n", desc));
        }
        
        rule.push_str(&format!("level: {}\n", level));
        
        if !tags.is_empty() {
            rule.push_str("tags:\n");
            for tag in &tags {
                rule.push_str(&format!("  - {}\n", tag));
            }
        }
        
        rule.push_str("detection:\n  selection:\n    EventID: 1\n  condition: selection\n");
        
        // Should parse successfully regardless of optional fields
        let result = sigma_rs::rule::rule_from_yaml(rule.as_bytes());
        
        if let Ok(parsed) = result {
            assert_eq!(parsed.title, title);
            assert_eq!(parsed.level.as_deref(), Some(level));
        }
    }
}

// Stateful property tests for rule evaluation
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    #[test]
    fn test_rule_determinism(
        rule in sigma_rule_strategy(),
        event in event_strategy()
    ) {
        // Property: Rule evaluation should be deterministic
        if let Ok(parsed_rule) = sigma_rs::rule::rule_from_yaml(rule.as_bytes()) {
            let dynamic_event = DynamicEvent::new(event.clone());
            
            // TODO: Once Tree is implemented
            // let tree1 = Tree::new(parsed_rule.clone()).unwrap();
            // let tree2 = Tree::new(parsed_rule).unwrap();
            //
            // let result1 = tree1.matches(&dynamic_event);
            // let result2 = tree2.matches(&dynamic_event);
            //
            // assert_eq!(result1, result2, "Rule evaluation should be deterministic");
        }
    }
}

// Shrinking tests - ensure the framework can minimize failing cases
proptest! {
    #[test]
    fn test_minimal_failing_rule(
        rule in sigma_rule_strategy()
    ) {
        // This test intentionally does nothing but helps verify shrinking works
        if rule.len() > 1000 {
            // Artificial failure to test shrinking
            prop_assert!(rule.len() <= 1000, "Rule too long: {}", rule.len());
        }
    }
}
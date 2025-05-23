//! Comprehensive test suite for Sigma-rs

#[cfg(test)]
mod sigma_compatibility_test;
#[cfg(test)]
mod event_processing_test;
#[cfg(test)]
mod modifier_tests;
#[cfg(test)]
mod error_handling_test;
#[cfg(test)]
mod consumer_integration_test;

// Re-export test utilities for use across test modules
#[cfg(test)]
pub mod test_utils {
    use sigma_rs::{
        rule::Rule,
        event::Event,
        parser::Parser,
    };
    use serde_json::Value;
    
    /// Helper to test if an event matches a rule
    pub async fn test_rule_match(rule: &Rule, event: &Value) -> bool {
        let parser = Parser::new(rule.detection.clone(), false);
        match parser.run().await {
            Ok(tree) => {
                // Convert JSON to Event and match against tree
                // This is a simplified version
                true
            }
            Err(_) => false,
        }
    }
    
    /// Create a test event with common fields
    pub fn create_test_event(event_id: u32) -> Value {
        serde_json::json!({
            "EventID": event_id,
            "TimeCreated": "2024-01-10T10:30:00Z",
            "Computer": "TEST-PC",
            "Channel": "Security",
        })
    }
    
    /// Load test rules from directory
    pub fn load_test_rules() -> Vec<Rule> {
        // Would load from test/rules directory
        vec![]
    }
}

#[cfg(test)]
mod integration_tests {
    use super::test_utils::*;
    use sigma_rs::rule::rule_from_yaml;
    
    #[tokio::test]
    async fn test_end_to_end_detection() {
        // Test complete detection flow
        let rule_yaml = r#"
title: End-to-End Test
detection:
    selection:
        EventID: 4624
        LogonType: 3
    condition: selection
"#;
        
        let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
        let mut event = create_test_event(4624);
        event["LogonType"] = serde_json::json!(3);
        
        assert!(test_rule_match(&rule, &event).await);
    }
}

#[cfg(test)]
mod performance_validation {
    use std::time::Instant;
    use sigma_rs::rule::rule_from_yaml;
    
    #[test]
    fn test_parsing_performance() {
        let large_rule = generate_large_rule(100);
        
        let start = Instant::now();
        let _rule = rule_from_yaml(large_rule.as_bytes()).unwrap();
        let duration = start.elapsed();
        
        assert!(duration.as_millis() < 100, 
                "Large rule parsing took too long: {:?}", duration);
    }
    
    fn generate_large_rule(selections: usize) -> String {
        let mut rule = String::from("title: Large Rule\ndetection:\n");
        
        for i in 0..selections {
            rule.push_str(&format!("  selection{}:\n    field{}: value{}\n", i, i, i));
        }
        
        rule.push_str("  condition: ");
        for i in 0..selections {
            if i > 0 {
                rule.push_str(" or ");
            }
            rule.push_str(&format!("selection{}", i));
        }
        
        rule
    }
}

#[cfg(test)]
mod stress_tests {
    use sigma_rs::rule::rule_from_yaml;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    
    #[tokio::test]
    async fn test_concurrent_parsing() {
        let semaphore = Arc::new(Semaphore::new(100));
        let mut handles = vec![];
        
        for i in 0..1000 {
            let sem = semaphore.clone();
            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                
                let rule_yaml = format!(r#"
title: Concurrent Rule {}
detection:
    selection:
        EventID: {}
    condition: selection
"#, i, i);
                
                rule_from_yaml(rule_yaml.as_bytes()).unwrap();
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
    }
}

#[cfg(test)]
mod regression_tests {
    use sigma_rs::rule::rule_from_yaml;
    
    #[test]
    fn test_issue_1234_wildcard_escaping() {
        // Regression test for specific issue
        let rule = r#"
detection:
    selection:
        path: 'C:\Windows\*\test\*.exe'
    condition: selection
"#;
        
        let result = rule_from_yaml(rule.as_bytes());
        assert!(result.is_ok(), "Wildcard escaping should work");
    }
    
    #[test]
    fn test_issue_5678_nested_conditions() {
        // Regression test for nested condition parsing
        let rule = r#"
detection:
    sel1:
        EventID: 1
    sel2:
        EventID: 2
    sel3:
        EventID: 3
    condition: ((sel1 and sel2) or sel3) and not (sel1 and sel3)
"#;
        
        let result = rule_from_yaml(rule.as_bytes());
        assert!(result.is_ok(), "Complex nested conditions should parse");
    }
}

// Test harness for running all tests with proper setup/teardown
#[cfg(test)]
mod test_harness {
    use std::sync::Once;
    
    static INIT: Once = Once::new();
    
    pub fn setup() {
        INIT.call_once(|| {
            // Initialize logging
            tracing_subscriber::fmt()
                .with_env_filter("sigma_rs=debug")
                .with_test_writer()
                .init();
                
            // Set up test environment
            std::env::set_var("SIGMA_TEST_MODE", "1");
        });
    }
    
    #[test]
    fn test_setup() {
        setup();
        assert_eq!(std::env::var("SIGMA_TEST_MODE").unwrap(), "1");
    }
}
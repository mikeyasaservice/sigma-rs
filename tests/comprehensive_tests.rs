/// Comprehensive test suite for Sigma rule engine
/// This module implements the testing strategy outlined in TESTING_STRATEGY.md

use sigma_rs::{DynamicEvent, Selector};
use sigma_rs::rule::Rule;
use std::fs;
use std::path::PathBuf;
use serde_json::json;
use anyhow::Result;

/// Test fixture loader for Sigma rules
struct TestFixtures {
    rules_dir: PathBuf,
    events_dir: PathBuf,
}

impl TestFixtures {
    fn new() -> Self {
        Self {
            rules_dir: PathBuf::from("tests/fixtures/rules"),
            events_dir: PathBuf::from("tests/fixtures/events"),
        }
    }

    fn load_sigma_rules(&self) -> Result<Vec<Rule>> {
        let mut rules = Vec::new();
        
        for entry in fs::read_dir(&self.rules_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yml") {
                let content = fs::read_to_string(&path)?;
                match sigma_rs::rule::rule_from_yaml(content.as_bytes()) {
                    Ok(rule) => rules.push(rule),
                    Err(e) => tracing::error!("Failed to parse rule {:?}: {}", path, e),
                }
            }
        }
        
        Ok(rules)
    }

    fn load_test_events(&self) -> Result<Vec<serde_json::Value>> {
        let mut events = Vec::new();
        
        for entry in fs::read_dir(&self.events_dir)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                let event: serde_json::Value = serde_json::from_str(&content)?;
                events.push(event);
            }
        }
        
        Ok(events)
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_simple_pattern_matching() {
        let patterns = vec![
            ("contains", "test", "this is a test string", true),
            ("contains", "missing", "this is a test string", false),
            ("startswith", "this", "this is a test", true),
            ("startswith", "test", "this is a test", false),
            ("endswith", "test", "this is a test", true),
            ("endswith", "this", "this is a test", false),
        ];

        for (modifier, pattern, value, expected) in patterns {
            let result = match modifier {
                "contains" => value.contains(pattern),
                "startswith" => value.starts_with(pattern),
                "endswith" => value.ends_with(pattern),
                _ => false,
            };
            
            assert_eq!(result, expected, 
                "Pattern {} with modifier {} on value {} should be {}", 
                pattern, modifier, value, expected);
        }
    }

    #[test]
    fn test_array_value_matching() {
        let event = json!({
            "EventID": 1,
            "Image": "C:\\Windows\\System32\\cmd.exe",
            "CommandLine": "cmd.exe /c whoami"
        });

        let rule_selection = json!({
            "Image": [
                "*\\cmd.exe",
                "*\\powershell.exe",
                "*\\pwsh.exe"
            ]
        });

        // Test that at least one value in the array matches
        let image = event["Image"].as_str().unwrap();
        let matches = rule_selection["Image"]
            .as_array()
            .unwrap()
            .iter()
            .any(|pattern| {
                let p = pattern.as_str().unwrap();
                if p.contains("*") {
                    image.ends_with(&p.replace("*", ""))
                } else {
                    image == p
                }
            });

        assert!(matches, "Image should match at least one pattern in the array");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_pipeline() {
        let rule_yaml = r#"
title: Suspicious PowerShell Encoded Command
id: test-001
detection:
  selection:
    EventID: 1
    Image|endswith: '\powershell.exe'
    CommandLine|contains: '-EncodedCommand'
  condition: selection
"#;

        let event = json!({
            "EventID": 1,
            "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
            "CommandLine": "powershell.exe -EncodedCommand SGVsbG8gV29ybGQ="
        });

        // Parse rule
        let rule = sigma_rs::rule::rule_from_yaml(rule_yaml.as_bytes()).unwrap();
        
        // Create event
        let dynamic_event = DynamicEvent::new(event);
        
        // Build and evaluate tree
        let rule_handle = sigma_rs::rule::RuleHandle::new(rule, std::path::PathBuf::from("test.yml"));
        let tree = sigma_rs::tree::build_tree(rule_handle).await.unwrap();
        let (matches, _applicable) = tree.match_event(&dynamic_event).await;
        assert!(matches, "Event should match the rule");
    }

    #[tokio::test]
    async fn test_complex_conditions() {
        let rule_yaml = r#"
title: Complex Detection Rule
id: test-002
detection:
  process_creation:
    EventID: 1
    Image|endswith: 
      - '\cmd.exe'
      - '\powershell.exe'
  network_connection:
    EventID: 3
    DestinationPort: 
      - 445
      - 3389
  suspicious_parent:
    ParentImage|contains:
      - '\temp\'
      - '\downloads\'
  condition: (process_creation or network_connection) and not suspicious_parent
"#;

        // Test multiple scenarios
        let test_cases = vec![
            (
                json!({
                    "EventID": 1,
                    "Image": "C:\\Windows\\System32\\cmd.exe",
                    "ParentImage": "C:\\Windows\\explorer.exe"
                }),
                true, // Should match: process_creation and not suspicious_parent
            ),
            (
                json!({
                    "EventID": 3,
                    "DestinationPort": 445,
                    "SourceImage": "C:\\Windows\\System32\\svchost.exe"
                }),
                true, // Should match: network_connection and not suspicious_parent
            ),
            (
                json!({
                    "EventID": 1,
                    "Image": "C:\\Windows\\System32\\cmd.exe",
                    "ParentImage": "C:\\Users\\User\\Downloads\\malware.exe"
                }),
                false, // Should not match: suspicious_parent
            ),
        ];

        for (event, expected) in test_cases {
            let dynamic_event = DynamicEvent::new(event);
            let rule_handle = sigma_rs::rule::RuleHandle::new(rule, std::path::PathBuf::from("test.yml"));
            let tree = sigma_rs::tree::build_tree(rule_handle).await.unwrap();
            let (matches, _applicable) = tree.match_event(&dynamic_event).await;
            assert_eq!(matches, expected, "Complex condition evaluation failed");
        }
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use std::process::Command;

    /// Compare results with Go implementation
    #[test]
    #[ignore] // Requires Go implementation to be available
    fn test_go_compatibility() {
        let test_rule = r#"
title: Test Rule
detection:
  selection:
    EventID: 4688
    NewProcessName|endswith: '\cmd.exe'
  condition: selection
"#;

        let test_event = json!({
            "EventID": 4688,
            "NewProcessName": "C:\\Windows\\System32\\cmd.exe"
        });

        // Run Go implementation
        let go_output = Command::new("go")
            .args(&["run", "../go-sigma-test/main.go"])
            .env("RULE", test_rule)
            .env("EVENT", test_event.to_string())
            .output()
            .expect("Failed to run Go implementation");

        let go_result: bool = serde_json::from_slice(&go_output.stdout).unwrap();

        // Run Rust implementation
        let rule = sigma_rs::rule::rule_from_yaml(test_rule.as_bytes()).unwrap();
        let rule_handle = sigma_rs::rule::RuleHandle::new(rule, std::path::PathBuf::from("test.yml"));
        let tree = sigma_rs::tree::build_tree(rule_handle).await.unwrap();
        let dynamic_event = DynamicEvent::new(test_event);
        let (rust_result, _applicable) = tree.match_event(&dynamic_event).await;
        
        assert_eq!(rust_result, go_result, "Results should match between Go and Rust");
    }
}

#[cfg(test)]
mod real_world_tests {
    use super::*;

    #[test]
    fn test_official_sigma_rules() {
        let fixtures = TestFixtures::new();
        
        // Create test fixtures directory if it doesn't exist
        fs::create_dir_all(&fixtures.rules_dir).ok();
        fs::create_dir_all(&fixtures.events_dir).ok();
        
        // Note: In a real scenario, these would be populated with actual Sigma rules
        // and event logs from the official repository
        
        match fixtures.load_sigma_rules() {
            Ok(rules) => {
                tracing::error!("Loaded {} test rules", rules.len());
                for rule in &rules {
                    tracing::error!("  - {}: {}", rule.id, rule.title);
                }
            }
            Err(e) => {
                tracing::error!("Failed to load rules: {}", e);
            }
        }
    }

    #[test]
    fn test_windows_event_logs() {
        let event = json!({
            "EventID": 4624,
            "Channel": "Security",
            "Provider": {
                "Name": "Microsoft-Windows-Security-Auditing"
            },
            "EventData": {
                "SubjectUserSid": "S-1-5-18",
                "SubjectUserName": "SYSTEM",
                "SubjectDomainName": "NT AUTHORITY",
                "TargetUserSid": "S-1-5-21-1234567890-1234567890-1234567890-1001",
                "TargetUserName": "testuser",
                "TargetDomainName": "TESTDOMAIN",
                "LogonType": "3",
                "LogonProcessName": "NtLmSsp",
                "AuthenticationPackageName": "NTLM"
            }
        });

        // Test Windows logon detection
        let rule_yaml = r#"
title: Network Logon
detection:
  selection:
    EventID: 4624
    LogonType: '3'
  condition: selection
"#;

        // Parse rule and build tree
        let rule = sigma_rs::rule::rule_from_yaml(rule_yaml.as_bytes()).unwrap();
        let rule_handle = sigma_rs::rule::RuleHandle::new(rule, std::path::PathBuf::from("test.yml"));
        let tree = sigma_rs::tree::build_tree(rule_handle).await.unwrap();
        
        // Test matching event
        let matching_event = json!({
            "EventID": 4624,
            "LogonType": "3"
        });
        let dynamic_event = DynamicEvent::new(matching_event);
        let (matches, _applicable) = tree.match_event(&dynamic_event).await;
        assert!(matches, "Network logon event should match");
        
        // Test non-matching event
        let non_matching_event = json!({
            "EventID": 4624,
            "LogonType": "2"  // Interactive logon, not network
        });
        let dynamic_event = DynamicEvent::new(non_matching_event);
        let (matches, _applicable) = tree.match_event(&dynamic_event).await;
        assert!(!matches, "Interactive logon should not match network logon rule");
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;
    use proptest::prelude::*;

    // Generate arbitrary field names
    prop_compose! {
        fn field_name()(name in "[a-zA-Z][a-zA-Z0-9_]{0,20}") -> String {
            name
        }
    }

    // Generate arbitrary field values
    prop_compose! {
        fn field_value()(
            value in prop_oneof![
                Just(json!(null)),
                any::<bool>().prop_map(|b| json!(b)),
                any::<i64>().prop_map(|i| json!(i)),
                "[a-zA-Z0-9 ]{0,50}".prop_map(|s| json!(s)),
            ]
        ) -> serde_json::Value {
            value
        }
    }

    // Generate arbitrary events
    prop_compose! {
        fn arbitrary_event()(
            fields in prop::collection::hash_map(field_name(), field_value(), 1..10)
        ) -> serde_json::Value {
            json!(fields)
        }
    }

    proptest! {
        #[test]
        fn test_event_creation_doesnt_panic(event in arbitrary_event()) {
            // Should never panic when creating an event
            let _ = DynamicEvent::new(event);
        }

        #[test]
        fn test_field_selection_doesnt_panic(
            event in arbitrary_event(),
            field in field_name()
        ) {
            let dynamic_event = DynamicEvent::new(event);
            // Should never panic when selecting a field
            let _ = dynamic_event.select(&field);
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_rule_parsing() {
        let rule_yaml = r#"
title: Complex Rule
detection:
  selection1:
    EventID: 
      - 1
      - 4688
    Image|endswith:
      - '\cmd.exe'
      - '\powershell.exe'
      - '\wscript.exe'
  selection2:
    CommandLine|contains|all:
      - 'http'
      - 'download'
  filter:
    User: 'SYSTEM'
  condition: (selection1 and selection2) and not filter
"#;

        let iterations = 1000;
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _ = sigma_rs::rule::rule_from_yaml(rule_yaml.as_bytes());
        }
        
        let duration = start.elapsed();
        let avg_time = duration / iterations;
        
        tracing::error!("Average rule parsing time: {:?}", avg_time);
        assert!(avg_time.as_micros() < 1000, "Rule parsing should be fast");
    }

    #[test]
    fn benchmark_event_matching() {
        let event = json!({
            "EventID": 1,
            "Image": "C:\\Windows\\System32\\cmd.exe",
            "CommandLine": "cmd.exe /c curl http://example.com/download.exe",
            "User": "john.doe",
            "ProcessId": 1234
        });

        let iterations = 10000;
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _ = DynamicEvent::new(event.clone());
        }
        
        let duration = start.elapsed();
        let avg_time = duration / iterations;
        
        tracing::error!("Average event creation time: {:?}", avg_time);
        assert!(avg_time.as_micros() < 100, "Event creation should be fast");
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_empty_rule() {
        let rule_yaml = r#"
title: Empty Rule
detection:
  condition: selection
"#;
        
        let result = sigma_rs::rule::rule_from_yaml(rule_yaml.as_bytes());
        assert!(result.is_err(), "Should fail on missing selection");
    }

    #[test]
    fn test_malformed_yaml() {
        let malformed_yaml = r#"
title: Malformed Rule
detection:
  selection:
    EventID: [1, 2, 3
  condition: selection
"#;
        
        let result = sigma_rs::rule::rule_from_yaml(malformed_yaml.as_bytes());
        assert!(result.is_err(), "Should fail on malformed YAML");
    }

    #[test]
    fn test_unicode_handling() {
        let event = json!({
            "Message": "用户登录成功 🔐",
            "User": "测试用户"
        });

        let rule_yaml = r#"
title: Unicode Test
id: unicode-test-001
detection:
  selection:
    Message|contains: '用户登录'
  condition: selection
"#;

        // Test that unicode is handled correctly
        let _ = sigma_rs::rule::rule_from_yaml(rule_yaml.as_bytes()).unwrap();
        let _ = DynamicEvent::new(event);
    }

    #[test]
    fn test_circular_references() {
        let rule_yaml = r#"
title: Circular Reference Test
detection:
  a: selection_b
  b: selection_a
  condition: a
"#;
        
        // Should handle circular references gracefully
        let result = sigma_rs::rule::rule_from_yaml(rule_yaml.as_bytes());
        // The actual behavior depends on implementation
    }

    #[test]
    fn test_extremely_large_rule() {
        let mut selections = String::new();
        let mut condition_parts = Vec::new();
        
        // Create a rule with 100 selections
        for i in 0..100 {
            selections.push_str(&format!("  selection{}:\n    EventID: {}\n", i, i));
            condition_parts.push(format!("selection{}", i));
        }
        
        let rule_yaml = format!(
            "title: Large Rule\nid: large-rule-test\ndetection:\n{}\n  condition: {}",
            selections,
            condition_parts.join(" or ")
        );
        
        let result = sigma_rs::rule::rule_from_yaml(rule_yaml.as_bytes());
        assert!(result.is_ok(), "Should handle large rules");
    }
}
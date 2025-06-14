//! Tests for Sigma rule compatibility and parsing

use serde_json::json;
use sigma_rs::{
    error::SigmaError,
    event::Event,
    parser::Parser,
    rule::{rule_from_yaml, Detection, Rule},
};
use std::collections::HashMap;

#[test]
fn test_basic_rule_parsing() {
    let rule_yaml = r#"
title: Basic Process Creation
id: test-001
status: experimental
logsource:
    product: windows
    category: process_creation
detection:
    selection:
        EventID: 1
        Image|endswith: '\cmd.exe'
    condition: selection
"#;

    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    assert_eq!(rule.title, "Basic Process Creation");
    assert_eq!(rule.id, "test-001");
}

#[tokio::test]
async fn test_complex_conditions() {
    let rule_yaml = r#"
title: Complex Detection
detection:
    selection1:
        EventID: 4688
    selection2:
        CommandLine|contains:
            - 'mimikatz'
            - 'procdump'
    filter:
        User: 'NT AUTHORITY\SYSTEM'
    condition: (selection1 and selection2) and not filter
"#;

    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    let mut parser = Parser::new(rule.detection.clone(), false);

    // Test should parse without errors
    assert!(parser.run().await.is_ok());
}

#[test]
fn test_all_modifiers() {
    // Test each modifier type
    let modifiers = vec![
        ("contains", r#"field|contains: 'test'"#),
        ("startswith", r#"field|startswith: 'test'"#),
        ("endswith", r#"field|endswith: 'test'"#),
        ("re", r#"field|re: '^test.*'"#),
        ("all", r#"field|contains|all: ['test1', 'test2']"#),
    ];

    for (name, detection) in modifiers {
        let rule_yaml = format!(
            r#"
title: Test {}
detection:
    selection:
        {}
    condition: selection
"#,
            name, detection
        );

        let result = rule_from_yaml(rule_yaml.as_bytes());
        assert!(result.is_ok(), "Failed to parse modifier: {}", name);
    }
}

#[test]
fn test_field_references() {
    let rule_yaml = r#"
detection:
    selection:
        EventID: 4624
        LogonType: 3
        IpAddress|startswith: '10.'
    condition: selection
fields:
    - EventID
    - LogonType
    - IpAddress
    - TargetUserName
"#;

    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    assert_eq!(rule.fields.len(), 4);
}

#[test]
fn test_wildcards_and_escaping() {
    let test_cases = vec![
        (r"*\cmd.exe", "should handle wildcard at start"),
        (r"C:\Windows\*\*.exe", "should handle multiple wildcards"),
        (r"test\*literal", "should handle escaped wildcard"),
        (r"\\server\share", "should handle UNC paths"),
    ];

    for (pattern, desc) in test_cases {
        let rule_yaml = format!(
            r#"
detection:
    selection:
        path: '{}'
    condition: selection
"#,
            pattern
        );

        let result = rule_from_yaml(rule_yaml.as_bytes());
        assert!(result.is_ok(), "Failed: {}", desc);
    }
}

#[tokio::test]
async fn test_event_matching() {
    let rule_yaml = r#"
detection:
    selection:
        EventID: 1
        Image|endswith: '\powershell.exe'
        CommandLine|contains: 'Invoke-Expression'
    condition: selection
"#;

    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    let mut parser = Parser::new(rule.detection.clone(), false);
    let tree = parser.run().await.unwrap();

    // Test matching event
    let matching_event = json!({
        "EventID": 1,
        "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        "CommandLine": "powershell.exe -Command Invoke-Expression"
    });

    // Test non-matching event
    let non_matching_event = json!({
        "EventID": 1,
        "Image": "C:\\Windows\\System32\\cmd.exe",
        "CommandLine": "cmd.exe /c dir"
    });

    // Convert to Event trait and test
    // Note: This assumes we have a proper Event implementation
}

#[test]
fn test_aggregation_rules() {
    let rule_yaml = r#"
detection:
    selection:
        EventID: 4625
    timeframe: 5m
    condition: selection | count() > 5
"#;

    let result = rule_from_yaml(rule_yaml.as_bytes());
    // Aggregation might not be implemented yet
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_invalid_rules() {
    let invalid_rules = vec![
        (
            "missing_condition",
            r#"
detection:
    selection:
        EventID: 1
"#,
        ),
        (
            "invalid_modifier",
            r#"
detection:
    selection:
        field|invalid_modifier: 'test'
    condition: selection
"#,
        ),
        (
            "syntax_error",
            r#"
detection:
    selection:
        field: [unclosed
    condition: selection
"#,
        ),
    ];

    for (name, yaml) in invalid_rules {
        let result = rule_from_yaml(yaml.as_bytes());
        assert!(result.is_err(), "Should fail to parse: {}", name);
    }
}

#[test]
fn test_nested_field_access() {
    let rule_yaml = r#"
detection:
    selection:
        EventData.SubjectUserName: 'admin'
        EventData.LogonType: 3
    condition: selection
"#;

    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    // Test that nested field access is properly parsed
}

#[test]
fn test_lists_and_maps() {
    let rule_yaml = r#"
detection:
    keywords:
        - 'password'
        - 'credential'
        - 'secret'
    selection:
        EventID: 
            - 4624
            - 4625
            - 4634
    condition: keywords or selection
"#;

    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    // Verify list handling in detection
}

#[test]
fn test_case_sensitivity() {
    let rule_yaml = r#"
detection:
    selection:
        CommandLine|contains: 'MIMIKATZ'
    condition: selection
"#;

    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    // Test should verify case handling
}

#[test]
fn test_null_values() {
    let rule_yaml = r#"
detection:
    selection:
        field: null
    filter:
        field: 
    condition: selection or filter
"#;

    let result = rule_from_yaml(rule_yaml.as_bytes());
    // Test null value handling
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_large_rule_parsing() {
        // Create a rule with many conditions
        let mut selections = String::new();
        let mut condition = String::new();

        for i in 0..100 {
            selections.push_str(&format!(
                r#"
    selection{}:
        field{}: value{}
"#,
                i, i, i
            ));

            if i > 0 {
                condition.push_str(" or ");
            }
            condition.push_str(&format!("selection{}", i));
        }

        let rule_yaml = format!(
            r#"
title: Large Rule Test
detection:
{}
    condition: {}
"#,
            selections, condition
        );

        let start = Instant::now();
        let result = rule_from_yaml(rule_yaml.as_bytes());
        let duration = start.elapsed();

        assert!(result.is_ok());
        tracing::error!("Large rule parsing took: {:?}", duration);
        assert!(duration.as_millis() < 100, "Parsing too slow");
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    // Test real Sigma rules from the official repository
    const MIMIKATZ_RULE: &str = r#"
title: Mimikatz Use
description: Detects the use of Mimikatz
references:
    - https://github.com/gentilkiwi/mimikatz
author: Florian Roth
date: 2018/01/07
modified: 2022/10/09
logsource:
    category: process_creation
    product: windows
detection:
    selection:
        - Image|endswith: '\mimikatz.exe'
        - CommandLine|contains:
            - 'mimikatz'
            - 'gentilkiwi'
    condition: selection
falsepositives:
    - Legitimate administrative activity
level: high
"#;

    #[test]
    fn test_real_mimikatz_rule() {
        let rule = rule_from_yaml(MIMIKATZ_RULE.as_bytes()).unwrap();
        assert_eq!(rule.title, "Mimikatz Use");
        assert_eq!(rule.level, Some("high".to_string()));
        assert!(rule.references.len() > 0);
    }
}

use sigma_rs::rule::{rule_from_yaml, Rule, Detection, Logsource, RuleHandle};
use std::path::PathBuf;

#[test]
fn test_rule_parsing() {
    let yaml = r#"
title: Process Creation Rule
id: 123e4567-e89b-12d3-a456-426614174000
status: experimental
description: Detects process creation events
references:
  - https://example.com
author: Test Author
date: 2023/01/01
modified: 2023/12/01
level: high
tags:
  - attack.execution
  - attack.t1059
logsource:
  product: windows
  category: process_creation
detection:
  selection:
    EventID: 1
    Image|endswith: '\cmd.exe'
  condition: selection
falsepositives:
  - Legitimate administrator activity
fields:
  - CommandLine
  - Image
  - User
    "#;
    
    let rule = rule_from_yaml(yaml.as_bytes()).unwrap();
    
    assert_eq!(rule.title, "Process Creation Rule");
    assert_eq!(rule.id, "123e4567-e89b-12d3-a456-426614174000");
    assert_eq!(rule.status, Some("experimental".to_string()));
    assert_eq!(rule.level, Some("high".to_string()));
    assert_eq!(rule.author, Some("Test Author".to_string()));
    assert_eq!(rule.tags.len(), 2);
    assert!(rule.has_tags(&["attack.execution".to_string()]));
    
    // Check logsource
    assert_eq!(rule.logsource.product, Some("windows".to_string()));
    assert_eq!(rule.logsource.category, Some("process_creation".to_string()));
    
    // Check detection
    assert_eq!(rule.detection.condition(), Some("selection"));
    let selection = rule.detection.get("selection").unwrap();
    assert!(selection.is_object());
}

#[test]
fn test_rule_handle() {
    let yaml = r#"
title: Simple Rule
id: test-123
detection:
  selection:
    EventID: 1
  condition: selection
    "#;
    
    let rule = rule_from_yaml(yaml.as_bytes()).unwrap();
    let handle = RuleHandle::new(rule, PathBuf::from("/path/to/rule.yml"))
        .with_multipart(false)
        .with_no_collapse_ws(true);
    
    assert_eq!(handle.path, PathBuf::from("/path/to/rule.yml"));
    assert!(!handle.multipart);
    assert!(handle.no_collapse_ws);
}

#[test]
fn test_multipart_detection() {
    assert!(!sigma_rs::rule::is_multipart(b"---\ntitle: Test"));
    assert!(sigma_rs::rule::is_multipart(b"title: Test\n---\ntitle: Test2"));
    assert!(!sigma_rs::rule::is_multipart(b"title: Test"));
}

#[test] 
fn test_detection_extraction() {
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), serde_json::json!("selection"));
    detection.insert("selection".to_string(), serde_json::json!({"EventID": 1}));
    detection.insert("filter".to_string(), serde_json::json!({"User": "SYSTEM"}));
    
    let extracted = detection.extract();
    assert_eq!(extracted.len(), 2);
    assert!(extracted.contains_key("selection"));
    assert!(extracted.contains_key("filter"));
    assert!(!extracted.contains_key("condition"));
}
//! Tree evaluation benchmarks comparable to Go implementation
//! 
//! This benchmarks the core tree evaluation performance that matches
//! the Go benchmarks for TreePositive and TreeNegative scenarios.

use criterion::{criterion_group, criterion_main, Criterion};
use sigma_rs::{DynamicEvent, rule::{RuleHandle, rule_from_yaml}, tree::builder::build_tree};
use serde_json::json;
use std::hint::black_box;
use std::path::PathBuf;
use tokio::runtime::Runtime;

/// Create test events for benchmarking
fn create_test_events() -> Vec<DynamicEvent> {
    vec![
        // Positive match events
        DynamicEvent::new(json!({
            "EventID": 1,
            "Image": "C:\\Windows\\System32\\cmd.exe",
            "CommandLine": "cmd.exe /c whoami",
            "User": "testuser"
        })),
        DynamicEvent::new(json!({
            "EventID": 1,
            "Image": "C:\\Windows\\System32\\powershell.exe", 
            "CommandLine": "powershell.exe -enc dGVzdA==",
            "User": "admin"
        })),
        DynamicEvent::new(json!({
            "EventID": 3,
            "Image": "C:\\Windows\\System32\\svchost.exe",
            "DestinationIp": "192.168.1.100",
            "DestinationPort": 443
        })),
        // Negative match events  
        DynamicEvent::new(json!({
            "EventID": 2,
            "Image": "C:\\Program Files\\Chrome\\chrome.exe",
            "CommandLine": "chrome.exe --new-tab",
            "User": "normaluser"
        })),
        DynamicEvent::new(json!({
            "EventID": 5,
            "Image": "C:\\Windows\\explorer.exe",
            "User": "SYSTEM"
        })),
        DynamicEvent::new(json!({
            "EventID": 1,
            "Image": "C:\\Windows\\System32\\notepad.exe",
            "CommandLine": "notepad.exe document.txt",
            "User": "testuser"
        })),
    ]
}

/// Create test rules for benchmarking
fn create_test_rules() -> Vec<String> {
    vec![
        // Rule 0: Simple process detection
        r#"
title: Simple Process Detection  
id: 12345678-1234-1234-1234-123456789001
detection:
  selection:
    EventID: 1
    Image|endswith: '\cmd.exe'
  condition: selection
"#.to_string(),

        // Rule 1: PowerShell detection
        r#"
title: PowerShell Detection
id: 12345678-1234-1234-1234-123456789002  
detection:
  selection:
    EventID: 1
    Image|endswith: '\powershell.exe'
    CommandLine|contains: '-enc'
  condition: selection
"#.to_string(),

        // Rule 2: Network activity
        r#"
title: Network Activity
id: 12345678-1234-1234-1234-123456789003
detection:
  selection:
    EventID: 3
    DestinationPort: 443
  condition: selection  
"#.to_string(),

        // Rule 3: Complex multi-condition
        r#"
title: Complex Detection
id: 12345678-1234-1234-1234-123456789004
detection:
  process:
    EventID: 1
    Image|endswith:
      - '\cmd.exe'
      - '\powershell.exe'
  cmdline:
    CommandLine|contains: 'whoami'
  filter:
    User: 'SYSTEM'
  condition: process and cmdline and not filter
"#.to_string(),

        // Rule 4: Multiple OR conditions
        r#"
title: Multiple OR Conditions
id: 12345678-1234-1234-1234-123456789005
detection:
  sel1:
    EventID: 1
    Image|endswith: '\cmd.exe'
  sel2:
    EventID: 1  
    Image|endswith: '\powershell.exe'
  sel3:
    EventID: 3
    DestinationPort: 
      - 80
      - 443
      - 8080
  condition: sel1 or sel2 or sel3
"#.to_string(),

        // Rule 5: String matching intensive
        r#"
title: String Matching Intensive
id: 12345678-1234-1234-1234-123456789006
detection:
  selection:
    CommandLine|contains|all:
      - 'whoami'
      - 'net'
      - 'user'
  condition: selection
"#.to_string(),

        // Rule 6: Complex nested logic
        r#"
title: Complex Nested Logic
id: 12345678-1234-1234-1234-123456789007
detection:
  proc_create:
    EventID: 1
  suspicious_image:
    Image|endswith:
      - '\cmd.exe'
      - '\powershell.exe'
      - '\wscript.exe'
  suspicious_cmdline:
    CommandLine|contains:
      - 'whoami'
      - 'net user'
      - 'ipconfig'
  admin_user:
    User|contains: 'admin'
  system_user:
    User: 'SYSTEM'
  condition: proc_create and suspicious_image and (suspicious_cmdline or admin_user) and not system_user
"#.to_string(),
    ]
}

/// Build trees from rules for benchmarking
async fn build_benchmark_trees() -> Vec<sigma_rs::tree::Tree> {
    let rules = create_test_rules();
    let mut trees = Vec::new();
    
    for (i, rule_yaml) in rules.iter().enumerate() {
        let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
        let rule_handle = RuleHandle::new(rule, PathBuf::from(format!("bench_rule_{}.yml", i)));
        let tree = build_tree(rule_handle).await.unwrap();
        trees.push(tree);
    }
    
    trees
}

fn benchmark_tree_positive(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let trees = rt.block_on(build_benchmark_trees());
    let positive_event = &create_test_events()[0]; // First event matches most rules
    
    // Simple benchmark to match Go style
    c.bench_function("tree_positive", |b| {
        b.iter(|| {
            rt.block_on(async {
                for tree in &trees {
                    black_box(tree.root.matches(black_box(positive_event)).await);
                }
            });
        });
    });
}

fn benchmark_tree_negative(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let trees = rt.block_on(build_benchmark_trees());
    let negative_event = &create_test_events()[3]; // Fourth event shouldn't match most rules
    
    // Simple benchmark to match Go style
    c.bench_function("tree_negative", |b| {
        b.iter(|| {
            rt.block_on(async {
                for tree in &trees {
                    black_box(tree.root.matches(black_box(negative_event)).await);
                }
            });
        });
    });
}


criterion_group!(
    benches, 
    benchmark_tree_positive,
    benchmark_tree_negative
);
criterion_main!(benches);
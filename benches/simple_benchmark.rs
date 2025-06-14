//! Simple performance benchmark to compare with Go implementation

use criterion::{criterion_group, criterion_main, Criterion};
use sigma_rs::{DynamicEvent, rule::{RuleHandle, rule_from_yaml}, tree::builder::build_tree};
use serde_json::json;
use std::hint::black_box;
use std::path::PathBuf;
use tokio::runtime::Runtime;

/// Create a simple test rule
fn create_test_rule() -> String {
    r#"
title: Simple Process Detection  
id: 12345678-1234-1234-1234-123456789001
detection:
  selection:
    EventID: 1
    Image|endswith: '\cmd.exe'
  condition: selection
"#.to_string()
}

/// Create test events
fn create_test_events() -> Vec<DynamicEvent> {
    vec![
        // Positive match event
        DynamicEvent::new(json!({
            "EventID": 1,
            "Image": "C:\\Windows\\System32\\cmd.exe",
            "CommandLine": "cmd.exe /c whoami",
            "User": "testuser"
        })),
        // Negative match event
        DynamicEvent::new(json!({
            "EventID": 2,
            "Image": "C:\\Program Files\\Chrome\\chrome.exe",
            "CommandLine": "chrome.exe --new-tab",
            "User": "normaluser"
        })),
    ]
}

fn benchmark_tree_evaluation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // Build tree
    let rule_yaml = create_test_rule();
    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    let rule_handle = RuleHandle::new(rule, PathBuf::from("bench.yml"));
    let tree = rt.block_on(build_tree(rule_handle)).unwrap();
    
    let events = create_test_events();
    let positive_event = &events[0];
    let negative_event = &events[1];
    
    // Benchmark positive matches (should match)
    c.bench_function("tree_positive", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(tree.root.matches(black_box(positive_event)).await)
            })
        });
    });
    
    // Benchmark negative matches (should not match)
    c.bench_function("tree_negative", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(tree.root.matches(black_box(negative_event)).await)
            })
        });
    });
}

fn benchmark_event_creation(c: &mut Criterion) {
    c.bench_function("event_creation", |b| {
        b.iter(|| {
            let data = json!({
                "EventID": 1,
                "Image": "C:\\Windows\\System32\\cmd.exe",
                "CommandLine": "cmd.exe /c whoami",
                "User": "testuser"
            });
            black_box(DynamicEvent::new(data));
        });
    });
}

fn benchmark_rule_parsing(c: &mut Criterion) {
    let rule_yaml = create_test_rule();
    
    c.bench_function("rule_parsing", |b| {
        b.iter(|| {
            black_box(rule_from_yaml(rule_yaml.as_bytes()).unwrap());
        });
    });
}

criterion_group!(
    benches, 
    benchmark_tree_evaluation,
    benchmark_event_creation,
    benchmark_rule_parsing
);
criterion_main!(benches);
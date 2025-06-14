//! Micro benchmark for direct performance measurement

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sigma_rs::{DynamicEvent, rule::{RuleHandle, rule_from_yaml}, tree::builder::build_tree};
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;
use tokio::runtime::Runtime;

fn bench_tree_evaluation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // Create a simple rule
    let rule_yaml = r#"
title: Simple Process Detection  
id: 12345678-1234-1234-1234-123456789001
detection:
  selection:
    EventID: 1
    Image|endswith: '\cmd.exe'
  condition: selection
"#;
    
    // Build tree once
    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    let rule_handle = RuleHandle::new(rule, PathBuf::from("bench.yml"));
    let tree = rt.block_on(build_tree(rule_handle)).unwrap();
    
    // Create test event
    let event = DynamicEvent::new(json!({
        "EventID": 1,
        "Image": "C:\\Windows\\System32\\cmd.exe",
        "CommandLine": "cmd.exe /c whoami",
        "User": "testuser"
    }));
    
    c.bench_function("tree_evaluation", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(tree.root.matches(black_box(&event)).await).unwrap()
            })
        })
    });
}

fn bench_event_creation(c: &mut Criterion) {
    c.bench_function("event_creation", |b| {
        b.iter(|| {
            let data = json!({
                "EventID": 1,
                "Image": "C:\\Windows\\System32\\cmd.exe",
                "CommandLine": "cmd.exe /c whoami",
                "User": "testuser"
            });
            black_box(DynamicEvent::new(data))
        })
    });
}

fn bench_rule_parsing(c: &mut Criterion) {
    let rule_yaml = r#"
title: Simple Process Detection  
id: 12345678-1234-1234-1234-123456789001
detection:
  selection:
    EventID: 1
    Image|endswith: '\cmd.exe'
  condition: selection
"#;
    
    c.bench_function("rule_parsing", |b| {
        b.iter(|| {
            black_box(rule_from_yaml(rule_yaml.as_bytes()).unwrap())
        })
    });
}

// Manual benchmark function to show actual timings
fn manual_benchmark() {
    println!("Manual Benchmark Results:");
    println!("========================");
    
    let rt = Runtime::new().unwrap();
    
    // Setup
    let rule_yaml = r#"
title: Simple Process Detection  
id: 12345678-1234-1234-1234-123456789001
detection:
  selection:
    EventID: 1
    Image|endswith: '\cmd.exe'
  condition: selection
"#;
    
    let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
    let rule_handle = RuleHandle::new(rule, PathBuf::from("bench.yml"));
    let tree = rt.block_on(build_tree(rule_handle)).unwrap();
    
    let event = DynamicEvent::new(json!({
        "EventID": 1,
        "Image": "C:\\Windows\\System32\\cmd.exe",
        "CommandLine": "cmd.exe /c whoami",
        "User": "testuser"
    }));
    
    // Warm up
    for _ in 0..1000 {
        rt.block_on(async {
            let _ = tree.root.matches(&event).await;
        });
    }
    
    // Benchmark tree evaluation
    let iterations = 100_000;
    let start = Instant::now();
    for _ in 0..iterations {
        rt.block_on(async {
            let _ = tree.root.matches(&event).await;
        });
    }
    let duration = start.elapsed();
    let avg_per_op = duration / iterations;
    
    println!("Tree Evaluation:");
    println!("  Total time: {:?}", duration);
    println!("  Per operation: {:?}", avg_per_op);
    println!("  Operations/sec: {:.0}", 1.0 / avg_per_op.as_secs_f64());
    println!("  Microseconds/op: {:.2}", avg_per_op.as_micros() as f64);
    
    // Compare with Go baseline (~1.3-1.5μs per operation)
    let go_baseline_us = 1.4; // Average from Go benchmarks
    let rust_us = avg_per_op.as_micros() as f64;
    let speedup = go_baseline_us / rust_us;
    
    println!("\nComparison with Go:");
    println!("  Go baseline: {:.2}μs per operation", go_baseline_us);
    println!("  Rust: {:.2}μs per operation", rust_us);
    if speedup > 1.0 {
        println!("  Rust is {:.2}x FASTER than Go", speedup);
    } else {
        println!("  Rust is {:.2}x slower than Go", 1.0 / speedup);
    }
}

criterion_group!(benches, bench_tree_evaluation, bench_event_creation, bench_rule_parsing);
criterion_main!(benches);

#[test]
fn run_manual_benchmark() {
    manual_benchmark();
}
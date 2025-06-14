//! Simple performance demonstration

use sigma_rs::{DynamicEvent, rule::{RuleHandle, rule_from_yaml}, tree::builder::build_tree};
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Sigma-rs Performance Benchmark");
    println!("==============================");
    
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
    
    // Build tree
    let rule = rule_from_yaml(rule_yaml.as_bytes())?;
    let rule_handle = RuleHandle::new(rule, PathBuf::from("bench.yml"));
    let tree = build_tree(rule_handle).await?;
    
    // Create test event
    let event = DynamicEvent::new(json!({
        "EventID": 1,
        "Image": "C:\\Windows\\System32\\cmd.exe",
        "CommandLine": "cmd.exe /c whoami",
        "User": "testuser"
    }));
    
    // Warm up
    for _ in 0..1000 {
        let _ = tree.root.matches(&event).await;
    }
    
    // Benchmark tree evaluation
    let iterations = 100_000;
    println!("Running {} iterations...", iterations);
    
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = tree.root.matches(&event).await;
    }
    let duration = start.elapsed();
    let avg_per_op = duration / iterations;
    
    println!("\nResults:");
    println!("  Total time: {:?}", duration);
    println!("  Average per operation: {:?}", avg_per_op);
    println!("  Operations per second: {:.0}", 1.0 / avg_per_op.as_secs_f64());
    println!("  Microseconds per operation: {:.2}", avg_per_op.as_micros() as f64);
    
    // Compare with Go baseline
    let go_baseline_us = 1.4; // Go benchmark baseline (~1.3-1.5μs)
    let rust_us = avg_per_op.as_micros() as f64;
    
    println!("\nComparison with Go baseline:");
    println!("  Go: {:.2}μs per operation", go_baseline_us);
    println!("  Rust: {:.2}μs per operation", rust_us);
    
    if rust_us < go_baseline_us {
        let speedup = go_baseline_us / rust_us;
        println!("  🚀 Rust is {:.2}x FASTER than Go!", speedup);
    } else {
        let slowdown = rust_us / go_baseline_us;
        println!("  📉 Rust is {:.2}x slower than Go", slowdown);
    }
    
    // Test both positive and negative cases
    println!("\nDetailed Performance:");
    
    // Positive match (should match)
    let positive_event = DynamicEvent::new(json!({
        "EventID": 1,
        "Image": "C:\\Windows\\System32\\cmd.exe"
    }));
    
    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = tree.root.matches(&positive_event).await;
    }
    let pos_duration = start.elapsed();
    println!("  Positive matches: {:.2}μs per operation", pos_duration.as_micros() as f64 / 10_000.0);
    
    // Negative match (should not match)
    let negative_event = DynamicEvent::new(json!({
        "EventID": 2,
        "Image": "C:\\Program Files\\Chrome\\chrome.exe"
    }));
    
    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = tree.root.matches(&negative_event).await;
    }
    let neg_duration = start.elapsed();
    println!("  Negative matches: {:.2}μs per operation", neg_duration.as_micros() as f64 / 10_000.0);
    
    Ok(())
}
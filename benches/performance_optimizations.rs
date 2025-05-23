//! Benchmarks for performance optimizations in sigma-rs
//!
//! This benchmark suite measures the impact of the performance optimizations:
//! 1. Arc-based sharing to reduce cloning in parallel rule evaluation  
//! 2. String interning and Cow<str> to reduce string allocations

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use sigma_rs::{
    event::DynamicEvent,
    pattern::{intern_pattern, global_interner_stats, escape_sigma_for_glob_cow},
    rule::rule_from_yaml,
    ruleset::RuleSet,
    SigmaEngineBuilder,
};
use serde_json::json;
use std::hint::black_box;
use tokio::runtime::Runtime;

/// Benchmark parallel rule evaluation with Arc sharing
async fn benchmark_parallel_rule_evaluation(num_rules: usize, num_events: usize) -> usize {
    // Create a ruleset with multiple similar rules
    let _builder = SigmaEngineBuilder::new();
    let mut ruleset = RuleSet::new();
    
    // Add multiple rules to test parallel evaluation
    for i in 0..num_rules {
        let rule_yaml = format!(r#"
        title: Test Rule {}
        id: test-rule-{}
        detection:
            selection:
                EventID: 1
                CommandLine|contains: 'test{}'
            condition: selection
        "#, i, i, i);
        
        let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
        ruleset.add_rule(rule).await.unwrap();
    }
    
    // Create test events
    let events: Vec<DynamicEvent> = (0..num_events)
        .map(|i| {
            DynamicEvent::new(json!({
                "EventID": 1,
                "CommandLine": format!("test{} command", i % num_rules)
            }))
        })
        .collect();
    
    // Evaluate all events against all rules
    let mut total_matches = 0;
    for event in &events {
        let result = ruleset.evaluate(event).await.unwrap();
        total_matches += result.matches.len();
    }
    
    total_matches
}

/// Benchmark string interning effectiveness
fn benchmark_string_interning(pattern_count: usize) -> usize {
    // Clear any existing interned strings by creating patterns that will be unique
    let unique_prefix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    
    // Create many duplicate patterns to test interning
    let mut interned_patterns = Vec::new();
    let base_patterns = [
        "cmd.exe",
        "powershell.exe", 
        "schtasks.exe",
        "net.exe",
        "wmic.exe",
    ];
    
    for i in 0..pattern_count {
        let pattern = format!("{}{}", base_patterns[i % base_patterns.len()], unique_prefix);
        let interned = intern_pattern(&pattern);
        interned_patterns.push(interned);
    }
    
    // Force multiple interns of the same patterns
    for _ in 0..3 {
        for i in 0..pattern_count.min(100) {
            let pattern = format!("{}{}", base_patterns[i % base_patterns.len()], unique_prefix);
            let interned = intern_pattern(&pattern);
            interned_patterns.push(interned);
        }
    }
    
    interned_patterns.len()
}

/// Benchmark escape function with Cow optimization
fn benchmark_escape_optimization(pattern_count: usize) -> usize {
    let patterns = [
        "simple_pattern",
        "another_simple",
        "test[bracket]pattern",
        "escape\\backslash",
        "complex{brace}pattern[with]multiple",
        "normal_text_no_escaping_needed",
        "more_normal_text",
        "basic_pattern",
    ];
    
    let mut escaped_count = 0;
    for _ in 0..pattern_count {
        for pattern in &patterns {
            let result = escape_sigma_for_glob_cow(pattern);
            if result.len() > 0 {
                escaped_count += 1;
            }
        }
    }
    
    escaped_count
}

fn bench_parallel_evaluation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("parallel_rule_evaluation");
    
    for num_rules in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("rules", num_rules),
            num_rules,
            |b, &num_rules| {
                b.iter(|| {
                    rt.block_on(benchmark_parallel_rule_evaluation(black_box(num_rules), black_box(10)))
                });
            },
        );
    }
    
    group.finish();
}

fn bench_string_interning(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_interning");
    
    // Benchmark before getting baseline stats
    let initial_stats = global_interner_stats();
    
    for size in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("patterns", size),
            size,
            |b, &size| {
                b.iter(|| benchmark_string_interning(black_box(size)));
            },
        );
    }
    
    // Report stats after benchmarking
    let final_stats = global_interner_stats();
    tracing::error!("String interner stats - Initial: {:?}, Final: {:?}", initial_stats, final_stats);
    
    group.finish();
}

fn bench_escape_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape_optimization");
    
    for size in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("escapes", size),
            size,
            |b, &size| {
                b.iter(|| benchmark_escape_optimization(black_box(size)));
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, bench_parallel_evaluation, bench_string_interning, bench_escape_optimization);
criterion_main!(benches);
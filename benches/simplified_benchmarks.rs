use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::hint::black_box;
use sigma_rs::{
    rule::{rule_from_yaml},
    pattern::{TextPatternModifier, factory::new_text_matcher},
};
use serde_json::json;

// Sample rules for benchmarking
const SIMPLE_RULE: &str = r#"
title: Simple Rule
detection:
    selection:
        EventID: 4688
        CommandLine|contains: 'powershell'
    condition: selection
"#;

const COMPLEX_RULE: &str = r#"
title: Complex Rule
detection:
    selection1:
        EventID: 
            - 4688
            - 4689
        CommandLine|contains:
            - 'powershell'
            - 'cmd.exe'
    selection2:
        UserName|contains: 'administrator'
        ProcessName|endswith: '.exe'
    condition: selection1 and selection2
"#;

fn benchmark_rule_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("rule_parsing");
    
    group.bench_function("parse_simple_rule", |b| {
        b.iter(|| {
            let rule = rule_from_yaml(black_box(SIMPLE_RULE.as_bytes())).unwrap();
            black_box(rule);
        });
    });
    
    group.bench_function("parse_complex_rule", |b| {
        b.iter(|| {
            let rule = rule_from_yaml(black_box(COMPLEX_RULE.as_bytes())).unwrap();
            black_box(rule);
        });
    });
    
    group.finish();
}

fn benchmark_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");
    
    // Contains pattern
    group.bench_function("contains_pattern", |b| {
        let pattern = new_text_matcher(
            TextPatternModifier::Contains,
            false,
            vec!["powershell".to_string()],
        ).unwrap();
        
        let test_strings = vec![
            "executing powershell.exe -ExecutionPolicy Bypass",
            "cmd.exe /c dir",
            "POWERSHELL.EXE -File script.ps1",
        ];
        
        b.iter(|| {
            for s in &test_strings {
                let result = pattern.r#match(black_box(s)).unwrap();
                black_box(result);
            }
        });
    });
    
    // Regex pattern
    group.bench_function("regex_pattern", |b| {
        let pattern = new_text_matcher(
            TextPatternModifier::Regex,
            false,
            vec![r"powershell\.exe\s+-\w+".to_string()],
        ).unwrap();
        
        let test_strings = vec![
            "executing powershell.exe -ExecutionPolicy Bypass",
            "cmd.exe /c dir",
            "powershell.exe -NoProfile -Command Get-Process",
        ];
        
        b.iter(|| {
            for s in &test_strings {
                let result = pattern.r#match(black_box(s)).unwrap();
                black_box(result);
            }
        });
    });
    
    group.finish();
}

fn benchmark_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    
    let sizes = [100, 1000, 10000];
    
    for size in sizes {
        group.throughput(Throughput::Elements(size as u64));
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                // Generate test data
                let events: Vec<serde_json::Value> = (0..size)
                    .map(|i| json!({
                        "EventID": if i % 2 == 0 { "4688" } else { "4624" },
                        "CommandLine": format!("powershell.exe -Command {}", i),
                        "UserName": format!("user{}", i % 10),
                    }))
                    .collect();
                
                b.iter(|| {
                    // Simulate processing events
                    for event in &events {
                        // Check EventID field
                        if let Some(event_id) = event.get("EventID").and_then(|v| v.as_str()) {
                            let matches = event_id == "4688";
                            black_box(matches);
                        }
                    }
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_rule_parsing,
    benchmark_pattern_matching,
    benchmark_throughput
);
criterion_main!(benches);
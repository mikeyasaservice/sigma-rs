use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::hint::black_box;
use sigma_rs::{
    rule::{Rule, rule_from_yaml, RuleHandle, Detection},
    parser::Parser,
    lexer::{Lexer, Token},
    pattern::{TextPatternModifier, factory::new_text_pattern},
    matcher::Matcher,
    event::{Event, DynamicEvent},
    tree::builder::build_tree,
};
use std::{time::Duration, collections::HashMap};
use serde_json::json;
use tokio::runtime::Runtime;

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
title: Complex Rule with Multiple Selections
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
    selection3:
        ParentProcessName: 'explorer.exe'
        ProcessName|contains:
            - 'reg.exe'
            - 'regedit.exe'
    condition: selection1 and (selection2 or selection3)
    timeframe: 10s
"#;

const RULE_WITH_AGGREGATION: &str = r#"
title: Rule with Aggregation
detection:
    selection:
        EventID: 4625
    condition: selection | count(UserName) > 10
    timeframe: 5m
"#;

fn benchmark_rule_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("rule_parsing");
    
    group.bench_function("parse_simple_rule", |b| {
        b.iter(|| {
            let rule = rule_from_yaml(std::hint::black_box(SIMPLE_RULE.as_bytes())).unwrap();
            std::hint::black_box(rule);
        });
    });
    
    group.bench_function("parse_complex_rule", |b| {
        b.iter(|| {
            let rule = rule_from_yaml(std::hint::black_box(COMPLEX_RULE.as_bytes())).unwrap();
            std::hint::black_box(rule);
        });
    });
    
    group.bench_function("parse_rule_with_aggregation", |b| {
        b.iter(|| {
            let rule = rule_from_yaml(std::hint::black_box(RULE_WITH_AGGREGATION.as_bytes())).unwrap();
            std::hint::black_box(rule);
        });
    });
    
    group.finish();
}

fn benchmark_lexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer");
    
    let conditions = [
        ("simple", "selection"),
        ("medium", "selection1 and selection2"),
        ("complex", "(selection1 or selection2) and not selection3"),
        ("nested", "((sel1 and sel2) or (sel3 and sel4)) and not sel5"),
    ];
    
    for (name, condition) in conditions {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut lexer = Lexer::new(black_box(condition));
                let tokens: Vec<Token> = lexer.collect();
                black_box(tokens);
            });
        });
    }
    
    group.finish();
}

fn benchmark_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");
    
    let conditions = [
        ("simple", "selection"),
        ("and_or", "selection1 and selection2 or selection3"),
        ("nested", "(selection1 or selection2) and (selection3 or selection4)"),
        ("not_complex", "not (selection1 and (selection2 or selection3))"),
    ];
    
    for (name, condition) in conditions {
        group.bench_function(name, |b| {
            b.iter(|| {
                // Create a minimal Detection object for parsing
                let mut detection = Detection::new();
                detection.insert("condition".to_string(), serde_json::json!(condition));
                
                let mut parser = Parser::new(detection, false);
                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    parser.run().await.unwrap();
                    let result = parser.result();
                    black_box(result);
                });
            });
        });
    }
    
    group.finish();
}

fn benchmark_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");
    
    // Contains pattern
    group.bench_function("contains_pattern", |b| {
        let pattern = new_text_pattern(
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
        let pattern = new_text_pattern(
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
    
    // Case insensitive pattern matching
    group.bench_function("case_insensitive", |b| {
        let pattern = new_text_pattern(
            TextPatternModifier::All,
            true,
            vec!["PowerShell".to_string(), "CMD".to_string(), "RUNDLL32".to_string()],
        ).unwrap();
        
        let test_strings = vec![
            "powershell cmd rundll32",
            "POWERSHELL CMD RUNDLL32",
            "PowerShell Cmd RunDLL32",
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

fn benchmark_event_matching(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("event_matching");
    
    // Benchmark different event sizes
    let small_event = json!({
        "EventID": "4688",
        "CommandLine": "powershell.exe -NoProfile",
        "UserName": "administrator"
    });
    
    let medium_event = json!({
        "EventID": "4688",
        "CommandLine": "powershell.exe -ExecutionPolicy Bypass -File script.ps1",
        "UserName": "administrator",
        "ProcessName": "powershell.exe",
        "ParentProcessName": "explorer.exe",
        "ProcessId": "1234",
        "ParentProcessId": "5678",
        "LogonId": "0x12345",
        "Timestamp": "2024-01-15T12:34:56Z"
    });
    
    let large_event = {
        let mut event = json!({
            "EventID": "4688",
            "CommandLine": "powershell.exe -ExecutionPolicy Bypass -File script.ps1",
            "UserName": "administrator",
        });
        
        // Add many fields
        for i in 0..50 {
            event[format!("Field{}", i)] = json!(format!("Value{}", i));
        }
        event
    };
    
    // Parse rules and build matchers
    let simple_rule = rule_from_yaml(SIMPLE_RULE.as_bytes()).unwrap();
    let complex_rule = rule_from_yaml(COMPLEX_RULE.as_bytes()).unwrap();
    
    // Create RuleHandles
    let simple_rule_handle = RuleHandle::new(simple_rule, std::path::PathBuf::from("benchmark"));
    let complex_rule_handle = RuleHandle::new(complex_rule, std::path::PathBuf::from("benchmark"));
    
    group.bench_function("small_event_simple_rule", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dynamic_event = DynamicEvent::new(small_event.clone());
                let tree = build_tree(simple_rule_handle.clone()).await.unwrap();
                let matcher = Matcher::new(tree);
                let result = matcher.eval(&dynamic_event);
                black_box(result);
            });
        });
    });
    
    group.bench_function("medium_event_complex_rule", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dynamic_event = DynamicEvent::new(medium_event.clone());
                let tree = build_tree(complex_rule_handle.clone()).await.unwrap();
                let matcher = Matcher::new(tree);
                let result = matcher.eval(&dynamic_event);
                black_box(result);
            });
        });
    });
    
    group.bench_function("large_event_complex_rule", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dynamic_event = DynamicEvent::new(large_event.clone());
                let tree = build_tree(complex_rule_handle.clone()).await.unwrap();
                let matcher = Matcher::new(tree);
                let result = matcher.eval(&dynamic_event);
                black_box(result);
            });
        });
    });
    
    group.finish();
}

fn benchmark_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("throughput");
    
    // Create different batch sizes for throughput testing
    let batch_sizes = [10, 100, 1000];
    
    for batch_size in batch_sizes {
        group.throughput(Throughput::Elements(batch_size as u64));
        
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &batch_size| {
                let events: Vec<_> = (0..batch_size)
                    .map(|i| json!({
                        "EventID": "4688",
                        "CommandLine": format!("powershell.exe -Command {}", i),
                        "UserName": format!("user{}", i % 10),
                        "ProcessId": format!("{}", 1000 + i),
                    }))
                    .collect();
                
                let rule_handle = rt.block_on(async {
                    let r = rule_from_yaml(SIMPLE_RULE.as_bytes()).unwrap();
                    RuleHandle::new(r, std::path::PathBuf::from("benchmark"))
                });
                
                b.iter(|| {
                    rt.block_on(async {
                        let tree = build_tree(rule_handle.clone()).await.unwrap();
                        let matcher = Matcher::new(tree);
                        
                        for event in &events {
                            let dynamic_event = DynamicEvent::new(event.clone());
                            let result = matcher.eval(&dynamic_event);
                            black_box(result);
                        }
                    });
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_rule_parsing,
    benchmark_lexer,
    benchmark_parser,
    benchmark_pattern_matching
    // benchmark_event_matching,
    // benchmark_throughput
);
criterion_main!(benches);
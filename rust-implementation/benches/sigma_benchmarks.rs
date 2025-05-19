use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use sigma_rs::{
    rule::{Rule, rule_from_yaml},
    parser::Parser,
    lexer::Lexer,
    pattern::{TextPatternModifier, string_matcher::new_string_matcher},
    matcher::RuleMatcher,
    event::Event,
};
use std::time::Duration;
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
title: Complex Rule
detection:
    selection1:
        EventID: 
            - 4688
            - 4689
            - 1
    selection2:
        CommandLine|contains|all:
            - 'invoke'
            - 'expression'
    selection3:
        Image|endswith:
            - '\powershell.exe'
            - '\pwsh.exe'
    filter1:
        User: 'NT AUTHORITY\SYSTEM'
    filter2:
        Image|startswith: 'C:\Program Files\'
    condition: (selection1 and selection2 and selection3) and not (filter1 or filter2)
"#;

const LARGE_RULE: &str = r#"
title: Large Rule with Many Conditions
detection:
    keywords:
        - mimikatz
        - procdump
        - lsadump
        - hashdump
        - secretsdump
        - ntlmrelay
        - rubeus
        - bloodhound
        - sharphound
        - kerbrute
    selection_process:
        EventID: 1
        Channel: 'Microsoft-Windows-Sysmon/Operational'
    selection_network:
        EventID: 3
        DestinationPort:
            - 445
            - 139
            - 135
            - 3389
    selection_file:
        EventID: 11
        TargetFilename|contains:
            - '.dmp'
            - 'lsass'
            - 'sam'
            - 'security'
            - 'ntds'
    filter:
        Image|endswith:
            - '\wmiprvse.exe'
            - '\svchost.exe'
            - '\services.exe'
    condition: (keywords or selection_process or selection_network or selection_file) and not filter
"#;

// Sample events for benchmarking
fn create_simple_event() -> serde_json::Value {
    json!({
        "EventID": 4688,
        "CommandLine": "powershell.exe -ExecutionPolicy Bypass",
        "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        "User": "CONTOSO\\john.doe"
    })
}

fn create_complex_event() -> serde_json::Value {
    json!({
        "EventID": 1,
        "Channel": "Microsoft-Windows-Sysmon/Operational",
        "CommandLine": "powershell.exe -Command Invoke-Expression (Get-Content script.ps1)",
        "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        "User": "CONTOSO\\john.doe",
        "ProcessId": 1234,
        "ParentProcessId": 5678,
        "ParentImage": "C:\\Windows\\System32\\cmd.exe",
        "WorkingDirectory": "C:\\Users\\john.doe",
        "IntegrityLevel": "Medium"
    })
}

fn create_large_event() -> serde_json::Value {
    let mut event = json!({
        "EventID": 1,
        "Channel": "Microsoft-Windows-Sysmon/Operational",
        "TimeCreated": "2024-01-10T10:30:00Z",
    });
    
    // Add many fields to test performance with large events
    if let Some(obj) = event.as_object_mut() {
        for i in 0..100 {
            obj.insert(format!("Field{}", i), json!(format!("Value{}", i)));
        }
    }
    
    event
}

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
    
    group.bench_function("parse_large_rule", |b| {
        b.iter(|| {
            let rule = rule_from_yaml(black_box(LARGE_RULE.as_bytes())).unwrap();
            black_box(rule);
        });
    });
    
    group.finish();
}

fn benchmark_lexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer");
    
    let conditions = vec![
        ("simple", "selection"),
        ("complex", "(selection1 and selection2) or (selection3 and not filter)"),
        ("nested", "((sel1 and sel2) or (sel3 and sel4)) and not (filter1 or filter2)"),
    ];
    
    for (name, condition) in conditions {
        group.bench_with_input(
            BenchmarkId::new("tokenize", name),
            condition,
            |b, condition| {
                b.iter(|| {
                    let mut lexer = Lexer::new(black_box(condition));
                    while lexer.next_token().is_some() {}
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_parser(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let mut group = c.benchmark_group("parser");
    
    group.bench_function("parse_simple_detection", |b| {
        let rule = rule_from_yaml(SIMPLE_RULE.as_bytes()).unwrap();
        b.to_async(&runtime).iter(|| async {
            let parser = Parser::new(black_box(rule.detection.clone()), false);
            parser.run().await.unwrap()
        });
    });
    
    group.bench_function("parse_complex_detection", |b| {
        let rule = rule_from_yaml(COMPLEX_RULE.as_bytes()).unwrap();
        b.to_async(&runtime).iter(|| async {
            let parser = Parser::new(black_box(rule.detection.clone()), false);
            parser.run().await.unwrap()
        });
    });
    
    group.finish();
}

fn benchmark_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");
    
    // Benchmark different pattern types
    let patterns = vec![
        ("exact", "powershell.exe", TextPatternModifier::None),
        ("contains", "powershell", TextPatternModifier::Contains),
        ("wildcard", "*\\powershell.exe", TextPatternModifier::None),
        ("regex", r"power\w+\.exe", TextPatternModifier::Regex),
    ];
    
    for (name, pattern, modifier) in patterns {
        group.bench_with_input(
            BenchmarkId::new("match", name),
            &(pattern, modifier),
            |b, (pattern, modifier)| {
                let matcher = new_string_matcher(
                    *modifier,
                    false,
                    false,
                    false,
                    vec![pattern.to_string()],
                ).unwrap();
                
                let test_strings = vec![
                    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                    "C:\\Windows\\System32\\cmd.exe",
                    "powershell.exe -ExecutionPolicy Bypass",
                ];
                
                b.iter(|| {
                    for s in &test_strings {
                        black_box(matcher.string_match(s));
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_event_matching(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let mut group = c.benchmark_group("event_matching");
    
    // Simple rule matching
    group.bench_function("match_simple_rule", |b| {
        let rule = rule_from_yaml(SIMPLE_RULE.as_bytes()).unwrap();
        let event = create_simple_event();
        
        b.to_async(&runtime).iter(|| async {
            let parser = Parser::new(rule.detection.clone(), false);
            let tree = parser.run().await.unwrap();
            // Actual matching would go here
            black_box(tree);
        });
    });
    
    // Complex rule matching
    group.bench_function("match_complex_rule", |b| {
        let rule = rule_from_yaml(COMPLEX_RULE.as_bytes()).unwrap();
        let event = create_complex_event();
        
        b.to_async(&runtime).iter(|| async {
            let parser = Parser::new(rule.detection.clone(), false);
            let tree = parser.run().await.unwrap();
            // Actual matching would go here
            black_box(tree);
        });
    });
    
    // Large event matching
    group.bench_function("match_large_event", |b| {
        let rule = rule_from_yaml(SIMPLE_RULE.as_bytes()).unwrap();
        let event = create_large_event();
        
        b.to_async(&runtime).iter(|| async {
            let parser = Parser::new(rule.detection.clone(), false);
            let tree = parser.run().await.unwrap();
            // Actual matching would go here
            black_box(tree);
        });
    });
    
    group.finish();
}

fn benchmark_batch_processing(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let mut group = c.benchmark_group("batch_processing");
    
    // Create a batch of events
    let events: Vec<_> = (0..1000)
        .map(|i| {
            json!({
                "EventID": if i % 2 == 0 { 4688 } else { 4689 },
                "CommandLine": format!("process_{}.exe", i),
                "Index": i
            })
        })
        .collect();
    
    group.throughput(Throughput::Elements(events.len() as u64));
    
    group.bench_function("batch_1000_events", |b| {
        let rule = rule_from_yaml(SIMPLE_RULE.as_bytes()).unwrap();
        
        b.to_async(&runtime).iter(|| async {
            let parser = Parser::new(rule.detection.clone(), false);
            let tree = parser.run().await.unwrap();
            
            // Simulate processing all events
            for event in &events {
                black_box(event);
                // Actual matching would go here
            }
        });
    });
    
    group.finish();
}

fn benchmark_rule_compilation(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let mut group = c.benchmark_group("rule_compilation");
    
    // Benchmark compiling multiple rules
    let rules = vec![
        SIMPLE_RULE,
        COMPLEX_RULE,
        LARGE_RULE,
    ];
    
    group.bench_function("compile_ruleset", |b| {
        b.to_async(&runtime).iter(|| async {
            let mut compiled = Vec::new();
            
            for rule_yaml in &rules {
                let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
                let parser = Parser::new(rule.detection.clone(), false);
                let tree = parser.run().await.unwrap();
                compiled.push((rule, tree));
            }
            
            black_box(compiled);
        });
    });
    
    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    
    // This is a simplified memory benchmark
    // In practice, you'd use a memory profiler
    group.bench_function("rule_memory_overhead", |b| {
        b.iter(|| {
            let mut rules = Vec::new();
            
            // Load many rules to measure memory impact
            for i in 0..100 {
                let rule_yaml = format!(r#"
title: Rule {}
detection:
    selection:
        EventID: {}
        Field{}: Value{}
    condition: selection
"#, i, i, i, i);
                
                let rule = rule_from_yaml(rule_yaml.as_bytes()).unwrap();
                rules.push(rule);
            }
            
            black_box(rules);
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_rule_parsing,
    benchmark_lexer,
    benchmark_parser,
    benchmark_pattern_matching,
    benchmark_event_matching,
    benchmark_batch_processing,
    benchmark_rule_compilation,
    benchmark_memory_usage
);

criterion_main!(benches);
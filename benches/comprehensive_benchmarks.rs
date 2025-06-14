/// Comprehensive benchmarks for Sigma rule engine
/// Measures performance of various components and compares with Go implementation

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::hint::black_box;
use sigma_rs::{DynamicEvent, Selector};
use serde_json::{json, Value};

/// Load test rules for benchmarking
fn load_benchmark_rules() -> Vec<String> {
    vec![
        // Simple rule
        r#"
title: Simple Process Detection
id: bench-001
detection:
  selection:
    EventID: 1
    Image|endswith: '\cmd.exe'
  condition: selection
"#.to_string(),
        
        // Complex rule with multiple conditions
        r#"
title: Complex Detection
id: bench-002
detection:
  process_creation:
    EventID: 1
    Image|endswith:
      - '\cmd.exe'
      - '\powershell.exe'
      - '\wscript.exe'
  network_activity:
    EventID: 3
    DestinationPort:
      - 445
      - 3389
      - 80
  suspicious_cmdline:
    CommandLine|contains|all:
      - 'whoami'
      - 'net user'
  filter:
    User: 'SYSTEM'
  condition: (process_creation or network_activity) and suspicious_cmdline and not filter
"#.to_string(),
        
        // Rule with many selections
        r#"
title: Many Selections
id: bench-003
detection:
  selection1:
    EventID: 1
    Image|contains: 'system32'
  selection2:
    EventID: 4688
    NewProcessName|contains: 'windows'
  selection3:
    CommandLine|contains: 'admin'
  selection4:
    ParentImage|endswith: '.exe'
  selection5:
    User|startswith: 'NT'
  selection6:
    IntegrityLevel: 'High'
  selection7:
    LogonId: '0x3e7'
  selection8:
    ProcessId|gte: 1000
  condition: all of selection*
"#.to_string(),
        
        // Rule with regex patterns
        r#"
title: Regex Detection
id: bench-004
detection:
  selection:
    CommandLine|re: '(wget|curl).*https?://.*\.(exe|dll|bat|ps1)'
    DestinationHostname|re: '^(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.'
  condition: selection
"#.to_string(),
    ]
}

/// Generate test events for benchmarking
fn generate_test_events(count: usize) -> Vec<Value> {
    let mut events = Vec::with_capacity(count);
    
    for i in 0..count {
        let event = match i % 4 {
            0 => json!({
                "EventID": 1,
                "Image": "C:\\Windows\\System32\\cmd.exe",
                "CommandLine": "cmd.exe /c whoami && net user",
                "User": "john.doe",
                "ProcessId": 1234 + i,
                "ParentImage": "C:\\Windows\\explorer.exe"
            }),
            1 => json!({
                "EventID": 3,
                "Image": "C:\\Program Files\\Chrome\\chrome.exe",
                "DestinationPort": 443,
                "DestinationHostname": "example.com",
                "SourcePort": 50000 + i
            }),
            2 => json!({
                "EventID": 4688,
                "NewProcessName": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                "CommandLine": "powershell.exe -NoProfile -ExecutionPolicy Bypass",
                "TokenElevationType": "%%1936",
                "ProcessId": 5000 + i
            }),
            _ => json!({
                "EventID": 7045,
                "ServiceName": "UpdateService",
                "ImagePath": "C:\\Temp\\update.exe",
                "ServiceType": "user mode service",
                "StartType": "auto start"
            }),
        };
        
        events.push(event);
    }
    
    events
}

fn benchmark_rule_parsing(c: &mut Criterion) {
    let rules = load_benchmark_rules();
    
    let mut group = c.benchmark_group("rule_parsing");
    
    for (i, rule) in rules.iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("parse", i), rule, |b, rule| {
            b.iter(|| {
                black_box(sigma_rs::rule::rule_from_yaml(rule.as_bytes()).unwrap());
            });
        });
    }
    
    group.finish();
}

fn benchmark_event_creation(c: &mut Criterion) {
    let events = generate_test_events(100);
    
    c.bench_function("event_creation", |b| {
        b.iter(|| {
            for event in &events {
                black_box(DynamicEvent::new(event.clone()));
            }
        });
    });
}

fn benchmark_field_selection(c: &mut Criterion) {
    let event = json!({
        "EventID": 1,
        "Process": {
            "Image": "C:\\Windows\\System32\\cmd.exe",
            "CommandLine": "cmd.exe /c dir",
            "Parent": {
                "Image": "C:\\Windows\\explorer.exe",
                "ProcessId": 1234
            }
        },
        "User": {
            "Name": "john.doe",
            "Domain": "CORP",
            "Sid": "S-1-5-21-123456"
        }
    });
    
    let dynamic_event = DynamicEvent::new(event);
    
    let mut group = c.benchmark_group("field_selection");
    
    // Benchmark direct field access
    group.bench_function("direct", |b| {
        b.iter(|| {
            black_box(dynamic_event.select("EventID"));
        });
    });
    
    // Benchmark nested field access
    group.bench_function("nested", |b| {
        b.iter(|| {
            black_box(dynamic_event.select("Process.Image"));
        });
    });
    
    // Benchmark deeply nested field access
    group.bench_function("deeply_nested", |b| {
        b.iter(|| {
            black_box(dynamic_event.select("Process.Parent.ProcessId"));
        });
    });
    
    group.finish();
}

fn benchmark_pattern_matching(c: &mut Criterion) {
    let test_strings = vec![
        "C:\\Windows\\System32\\cmd.exe",
        "This is a test string with some content",
        "powershell.exe -EncodedCommand SGVsbG8gV29ybGQ=",
        "https://malicious-site.com/download/payload.exe",
    ];
    
    let mut group = c.benchmark_group("pattern_matching");
    
    // Benchmark contains
    group.bench_function("contains", |b| {
        b.iter(|| {
            for s in &test_strings {
                black_box(s.contains("cmd"));
            }
        });
    });
    
    // Benchmark startswith
    group.bench_function("startswith", |b| {
        b.iter(|| {
            for s in &test_strings {
                black_box(s.starts_with("C:\\"));
            }
        });
    });
    
    // Benchmark endswith
    group.bench_function("endswith", |b| {
        b.iter(|| {
            for s in &test_strings {
                black_box(s.ends_with(".exe"));
            }
        });
    });
    
    // Benchmark regex
    let re = regex::Regex::new(r"https?://[^\s]+\.(exe|dll|bat)").unwrap();
    group.bench_function("regex", |b| {
        b.iter(|| {
            for s in &test_strings {
                black_box(re.is_match(s));
            }
        });
    });
    
    group.finish();
}

fn benchmark_rule_evaluation(c: &mut Criterion) {
    let _rules = load_benchmark_rules();
    let _events = generate_test_events(1000);
    
    let group = c.benchmark_group("rule_evaluation");
    
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let rules = load_benchmark_rules();
    let events = generate_test_events(1000);
    
    for (i, rule_str) in rules.iter().enumerate() {
        let rule = sigma_rs::rule::rule_from_yaml(rule_str.as_bytes()).unwrap();
        let rule_handle = sigma_rs::rule::RuleHandle::new(rule, std::path::PathBuf::from("bench.yml"));
        let tree = runtime.block_on(async {
            sigma_rs::tree::build_tree(rule_handle).await.unwrap()
        });
        
        group.bench_with_input(BenchmarkId::new("evaluate", i), &events, |b, events| {
            b.iter(|| {
                for event in events {
                    let dynamic_event = DynamicEvent::new(event.clone());
                    runtime.block_on(async {
                        black_box(tree.match_event(&dynamic_event).await);
                    });
                }
            });
        });
    }
    
    group.finish();
}

fn benchmark_ruleset_evaluation(c: &mut Criterion) {
    let _rules = load_benchmark_rules();
    let _events = generate_test_events(100);
    
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut ruleset = sigma_rs::ruleset::RuleSet::new();
    let rules = load_benchmark_rules();
    let events = generate_test_events(100);
    
    // Load rules into ruleset
    runtime.block_on(async {
        for rule_str in &rules {
            let rule = sigma_rs::rule::rule_from_yaml(rule_str.as_bytes()).unwrap();
            ruleset.add_rule(rule).await.unwrap();
        }
    });
    
    c.bench_function("ruleset_evaluation", |b| {
        b.iter(|| {
            runtime.block_on(async {
                for event in &events {
                    let dynamic_event = DynamicEvent::new(event.clone());
                    black_box(ruleset.evaluate(&dynamic_event).await.unwrap());
                }
            });
        });
    });
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");
    
    // Benchmark memory usage for large events
    group.bench_function("large_event", |b| {
        b.iter(|| {
            let mut event = json!({});
            for i in 0..1000 {
                event[format!("field_{}", i)] = json!(format!("value_{}", i));
            }
            black_box(DynamicEvent::new(event));
        });
    });
    
    // Benchmark memory usage for deeply nested events
    group.bench_function("deeply_nested_event", |b| {
        b.iter(|| {
            let mut event = json!({"value": "leaf"});
            for i in 0..100 {
                event = json!({format!("level_{}", i): event});
            }
            black_box(DynamicEvent::new(event));
        });
    });
    
    group.finish();
}

fn benchmark_concurrent_evaluation(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;
    
    let events = Arc::new(generate_test_events(1000));
    let num_threads = 4;
    
    c.bench_function("concurrent_evaluation", |b| {
        b.iter(|| {
            let mut handles = vec![];
            
            for _ in 0..num_threads {
                let events_clone = Arc::clone(&events);
                let handle = thread::spawn(move || {
                    for event in events_clone.iter() {
                        let _ = DynamicEvent::new(event.clone());
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

fn benchmark_edge_cases(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_cases");
    
    // Empty event
    group.bench_function("empty_event", |b| {
        b.iter(|| {
            black_box(DynamicEvent::new(json!({})));
        });
    });
    
    // Event with many fields
    group.bench_function("many_fields", |b| {
        b.iter(|| {
            let mut event = json!({});
            for i in 0..100 {
                event[format!("field_{}", i)] = json!(i);
            }
            black_box(DynamicEvent::new(event));
        });
    });
    
    // Event with Unicode
    group.bench_function("unicode_event", |b| {
        b.iter(|| {
            let event = json!({
                "用户": "测试用户",
                "メッセージ": "これはテストです",
                "🔐": "🔑"
            });
            black_box(DynamicEvent::new(event));
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_rule_parsing,
    benchmark_event_creation,
    benchmark_field_selection,
    benchmark_pattern_matching,
    benchmark_rule_evaluation,
    benchmark_ruleset_evaluation,
    benchmark_memory_usage,
    benchmark_concurrent_evaluation,
    benchmark_edge_cases
);
criterion_main!(benches);
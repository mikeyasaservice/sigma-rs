# Rust Sigma Engine - Comprehensive Testing Strategy

## 1. What We're Actually Trying to Validate

- **Correctness**: The Rust implementation should correctly parse and evaluate Sigma rules
- **Compatibility**: Output should match the Go implementation for the same rules and events
- **Reliability**: Handle edge cases, malformed rules, and invalid input gracefully
- **Performance**: Meet or exceed the Go implementation's performance
- **Real-world Usage**: Work with actual Sigma rules from the community repository

## 2. Testing Levels

### Unit Tests
- Test individual components in isolation
- Focus on small, specific functionality
- Fast execution, easy debugging

### Integration Tests
- Test the complete pipeline: lexer → parser → evaluation
- Use real Sigma rules with synthetic events
- Verify component interactions

### End-to-End Tests
- Use actual Sigma rules from the community repository
- Test against real-world event logs (Windows, Linux, etc.)
- Compare output with Go implementation

### Property-Based Tests
- Generate random valid and invalid rules
- Test parser robustness
- Ensure no panics or unexpected behavior

## 3. Test Structure

```
tests/
├── unit/
│   ├── lexer_tests.rs
│   ├── parser_tests.rs
│   ├── pattern_tests.rs
│   └── matcher_tests.rs
├── integration/
│   ├── pipeline_tests.rs
│   ├── rule_evaluation_tests.rs
│   └── edge_cases_tests.rs
├── compatibility/
│   ├── go_comparison_tests.rs
│   └── test_fixtures/
│       ├── rules/
│       └── events/
├── property_based/
│   ├── fuzzing_tests.rs
│   └── generators.rs
└── real_world/
    ├── sigma_rules_tests.rs
    └── benchmarks.rs
```

## 4. Key Test Scenarios

### 4.1 Parser Tests
```rust
// Test complex rule conditions
#[test]
fn test_complex_condition_parsing() {
    let rule = r#"
    detection:
      process_selection:
        EventID: 1
        Image|endswith:
          - '\cmd.exe'
          - '\powershell.exe'
      network_selection:
        EventID: 3
        DestinationPort: 
          - 445
          - 3389
      condition: process_selection or (network_selection and not filter)
    "#;
    
    // Parse and verify AST structure
}
```

### 4.2 Pattern Matching Tests
```rust
// Test various pattern types
#[test]
fn test_pattern_matching() {
    let patterns = vec![
        ("contains", "test string", "this is a test string", true),
        ("startswith", "prefix", "prefix_value", true),
        ("endswith", ".exe", "system32\\cmd.exe", true),
        ("re", r"\d{4}-\d{2}-\d{2}", "2023-12-25", true),
    ];
    
    for (pattern_type, pattern, value, expected) in patterns {
        // Test each pattern type
    }
}
```

### 4.3 Event Matching Tests
```rust
// Test event matching with real-world scenarios
#[test]
fn test_windows_process_creation() {
    let rule = load_sigma_rule("rules/windows/process_creation/proc_creation_win_susp_powershell_enc_cmd.yml");
    
    let event = json!({
        "EventID": 1,
        "Channel": "Microsoft-Windows-Sysmon/Operational",
        "CommandLine": "powershell.exe -EncodedCommand U3RhcnQtUHJvY2VzcyBjYWxjLmV4ZQ==",
        "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
    });
    
    assert!(rule.matches(&event));
}
```

### 4.4 Go Compatibility Tests
```rust
// Compare results with Go implementation
#[test]
fn test_go_compatibility() {
    let test_cases = load_compatibility_tests();
    
    for case in test_cases {
        let rust_result = evaluate_rule_rust(&case.rule, &case.event);
        let go_result = case.expected_result; // Pre-computed from Go
        
        assert_eq!(rust_result, go_result, 
            "Mismatch for rule {} with event {}", 
            case.rule_id, case.event_id);
    }
}
```

### 4.5 Edge Cases
```rust
// Test edge cases and error handling
#[test]
fn test_edge_cases() {
    // Empty rules
    test_empty_rule();
    
    // Malformed YAML
    test_malformed_yaml();
    
    // Missing required fields
    test_missing_fields();
    
    // Circular references
    test_circular_references();
    
    // Unicode handling
    test_unicode_patterns();
    
    // Extremely large rules
    test_large_rules();
}
```

### 4.6 Performance Benchmarks
```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_rule_parsing(c: &mut Criterion) {
    let rule = load_complex_rule();
    
    c.bench_function("parse complex rule", |b| {
        b.iter(|| parse_sigma_rule(&rule));
    });
}

fn benchmark_event_matching(c: &mut Criterion) {
    let ruleset = load_ruleset();
    let events = load_test_events();
    
    c.bench_function("match 1000 events", |b| {
        b.iter(|| {
            for event in &events {
                ruleset.evaluate(event);
            }
        });
    });
}
```

## 5. Test Data Sources

### 5.1 Official Sigma Rules
- Clone the sigma repository
- Use rules from different categories (process_creation, network_connection, etc.)
- Test both simple and complex rules

### 5.2 Synthetic Events
- Generate events that should match/not match specific rules
- Include edge cases (missing fields, wrong types, etc.)

### 5.3 Real Event Logs
- Windows Event Logs (Security, System, Sysmon)
- Linux audit logs
- Cloud provider logs (AWS CloudTrail, Azure Activity)

## 6. Property-Based Testing

```rust
use proptest::prelude::*;

// Generate random valid Sigma rules
prop_compose! {
    fn arbitrary_sigma_rule()(
        title in "[a-zA-Z0-9 ]+",
        event_id in 1..10000u32,
        fields in prop::collection::vec("[a-zA-Z0-9.]+", 1..10)
    ) -> String {
        format!(r#"
title: {}
detection:
  selection:
    EventID: {}
    {}
  condition: selection
"#, title, event_id, 
    fields.iter().map(|f| format!("{}: value", f)).collect::<Vec<_>>().join("\n    "))
    }
}

proptest! {
    #[test]
    fn test_parser_doesnt_panic(rule in arbitrary_sigma_rule()) {
        // Parser should never panic, even with random input
        let _ = parse_sigma_rule(&rule);
    }
}
```

## 7. Test Fixtures Management

### 7.1 Rule Fixtures
```rust
// Load test rules from YAML files
fn load_test_rules() -> Vec<TestRule> {
    let rules_dir = Path::new("tests/fixtures/rules");
    
    rules_dir.read_dir()
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if path.extension() == Some("yml") {
                Some(TestRule::from_file(&path))
            } else {
                None
            }
        })
        .collect()
}
```

### 7.2 Event Fixtures
```rust
// Load test events from JSON files
fn load_test_events() -> Vec<Event> {
    let events_file = Path::new("tests/fixtures/events/windows_events.json");
    let contents = fs::read_to_string(events_file).unwrap();
    serde_json::from_str(&contents).unwrap()
}
```

## 8. Continuous Testing

### 8.1 CI/CD Integration
- Run tests on every commit
- Benchmark performance to detect regressions
- Test against latest Sigma rules daily

### 8.2 Fuzz Testing
```bash
# Continuous fuzzing with cargo-fuzz
cargo fuzz run parser_fuzzer -- -max_len=10000 -timeout=5
```

### 8.3 Coverage Reporting
```bash
# Generate coverage reports
cargo tarpaulin --out Html --output-dir coverage
```

## 9. Test Implementation Plan

### Phase 1: Core Functionality (Week 1-2)
- [ ] Lexer unit tests
- [ ] Parser unit tests  
- [ ] Basic pattern matching tests
- [ ] Simple rule evaluation tests

### Phase 2: Advanced Features (Week 3-4)
- [ ] Complex condition tests
- [ ] Modifier tests (contains, endswith, etc.)
- [ ] Aggregation tests
- [ ] Error handling tests

### Phase 3: Compatibility (Week 5-6)
- [ ] Go comparison framework
- [ ] Run official Sigma rules
- [ ] Fix compatibility issues
- [ ] Performance benchmarks

### Phase 4: Robustness (Week 7-8)
- [ ] Property-based tests
- [ ] Fuzz testing setup
- [ ] Edge case coverage
- [ ] Real-world data tests

## 10. Success Criteria

1. **100% compatibility** with Go implementation on official Sigma rules
2. **No panics** on any input (valid or invalid)
3. **Performance parity** or better than Go implementation
4. **95%+ code coverage** for core modules
5. **Comprehensive documentation** for all test scenarios

## 11. Maintenance

- Regular updates when new Sigma rule features are added
- Automated testing against Sigma repository changes
- Performance regression detection
- Community-contributed test cases
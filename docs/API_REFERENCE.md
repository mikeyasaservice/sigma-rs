# Sigma-rs API Reference

## Quick Start

```rust
use sigma_rs::{DynamicEvent, rule_from_yaml, tree::build_tree};
use serde_json::json;

// Parse a Sigma rule
let rule = rule_from_yaml(include_bytes!("rule.yml"))?;

// Build detection tree
let rule_handle = RuleHandle::new(rule, PathBuf::from("rule.yml"));
let tree = build_tree(rule_handle).await?;

// Create and match event
let event = DynamicEvent::new(json!({
    "EventID": 1,
    "CommandLine": "powershell.exe"
}));

let (matched, applicable) = tree.match_event(&event).await;
```

## Core Components

### Rule Parsing

```rust
use sigma_rs::rule::{rule_from_yaml, Rule};

// Parse from YAML bytes
let rule: Rule = rule_from_yaml(yaml_bytes)?;

// Access rule properties
println!("Title: {}", rule.title);
println!("ID: {}", rule.id);
println!("Level: {:?}", rule.level);
```

### Event Types

The library provides several event implementations:

- `DynamicEvent` - JSON-based events (most common)
- `SimpleEvent` - Basic event for testing
- Custom events by implementing the `Event` trait

```rust
// Using DynamicEvent
let event = DynamicEvent::new(json!({
    "EventID": 4688,
    "ProcessName": "cmd.exe",
    "User": "admin"
}));

// Implementing custom event type
impl Event for MyEvent {
    fn id(&self) -> &str { &self.id }
    fn timestamp(&self) -> i64 { self.timestamp }
}

impl Keyworder for MyEvent {
    fn keywords(&self) -> (Vec<String>, bool) {
        // Return keyword fields
    }
}

impl Selector for MyEvent {
    fn select(&self, key: &str) -> (Option<Value>, bool) {
        // Return field value
    }
}
```

### RuleSet Management

```rust
use sigma_rs::ruleset::RuleSet;

// Create ruleset
let mut ruleset = RuleSet::new();

// Add rules
for rule in rules {
    ruleset.add_rule(rule).await?;
}

// Evaluate event against all rules
let result = ruleset.evaluate(&event).await?;
for match_result in result.matches {
    if match_result.matched {
        println!("Rule {} matched!", match_result.rule_id);
    }
}
```

### Kafka Integration

```rust
use sigma_rs::{SigmaEngineBuilder, KafkaConfig};

let config = KafkaConfig {
    brokers: "localhost:9092".to_string(),
    group_id: "sigma-processor".to_string(),
    topics: vec!["security-events".to_string()],
    batch_size: 1000,
    retry_policy: RetryPolicy::default(),
    dlq_topic: Some("dlq-events".to_string()),
    ..Default::default()
};

let engine = SigmaEngineBuilder::new()
    .add_rule_dir("/path/to/rules")
    .with_kafka(config)
    .with_worker_threads(4)
    .build()
    .await?;

// Run the engine
engine.run().await?;
```

## Pattern Modifiers

Sigma supports various field modifiers:

```yaml
detection:
  selection:
    # Exact match (default)
    EventID: 1
    
    # Contains
    CommandLine|contains: 'powershell'
    
    # Starts with
    ProcessName|startswith: 'C:\Windows'
    
    # Ends with
    FileName|endswith: '.exe'
    
    # Regular expression
    Path|re: '^C:\\Windows\\.*\.dll$'
    
    # All patterns must match
    CommandLine|all:
      - 'cmd'
      - '/c'
```

## Error Handling

All operations return `Result<T, SigmaError>`:

```rust
use sigma_rs::error::{SigmaError, Result};

match rule_from_yaml(yaml_bytes) {
    Ok(rule) => process_rule(rule),
    Err(SigmaError::YamlParse(e)) => eprintln!("YAML error: {}", e),
    Err(SigmaError::MissingCondition) => eprintln!("No condition in rule"),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Performance Tuning

### String Interning

For large deployments with many similar patterns:

```rust
use sigma_rs::pattern::intern::{set_interner_config, InternerConfig};

set_interner_config(InternerConfig {
    enabled: true,
    max_size: 10_000,
});
```

### Parallel Processing

```rust
let engine = SigmaEngineBuilder::new()
    .with_worker_threads(num_cpus::get())
    .build()
    .await?;
```

### Memory Limits

The library enforces various limits to prevent DoS:

- `MAX_RULES_PER_DIR`: 10,000 rules per directory
- `MAX_RULE_SIZE`: 1MB per rule file
- `MAX_TOKENS`: 10,000 tokens per condition
- `MAX_RECURSION_DEPTH`: 50 levels of nesting

## Logging

The library uses `tracing` for structured logging:

```rust
// Enable debug logging
tracing_subscriber::fmt()
    .with_env_filter("sigma_rs=debug")
    .init();

// Performance logging
tracing_subscriber::fmt()
    .with_env_filter("sigma_rs::consumer=trace")
    .init();
```

## Feature Flags

```toml
[dependencies]
# Core functionality only
sigma-rs = "1.0"

# With Kafka support
sigma-rs = { version = "1.0", features = ["kafka"] }

# With metrics
sigma-rs = { version = "1.0", features = ["metrics"] }

# All features
sigma-rs = { version = "1.0", features = ["all"] }
```
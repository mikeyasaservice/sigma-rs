# Sigma-rs: High-Performance Rust Implementation of Sigma Rules

[![CI](https://github.com/sigma-rs/sigma-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/sigma-rs/sigma-rs/actions/workflows/ci.yml)
[![Code Quality](https://github.com/sigma-rs/sigma-rs/actions/workflows/quality.yml/badge.svg)](https://github.com/sigma-rs/sigma-rs/actions/workflows/quality.yml)
[![Security](https://github.com/sigma-rs/sigma-rs/actions/workflows/security.yml/badge.svg)](https://github.com/sigma-rs/sigma-rs/actions/workflows/security.yml)
[![Coverage](https://codecov.io/gh/sigma-rs/sigma-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/sigma-rs/sigma-rs)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/sigma-rs.svg)](https://crates.io/crates/sigma-rs)
[![FOSSA Status](https://app.fossa.com/api/projects/custom%2B54172%2Fgithub.com%2Fseacurity%2Fsigma-rs.svg?type=shield&issueType=license)](https://app.fossa.com/projects/custom%2B54172%2Fgithub.com%2Fseacurity%2Fsigma-rs?ref=badge_shield&issueType=license)
[![FOSSA Status](https://app.fossa.com/api/projects/custom%2B54172%2Fgithub.com%2Fseacurity%2Fsigma-rs.svg?type=shield&issueType=security)](https://app.fossa.com/projects/custom%2B54172%2Fgithub.com%2Fseacurity%2Fsigma-rs?ref=badge_shield&issueType=security)

A production-ready Rust implementation of the Sigma rule engine with Redpanda/Kafka integration, designed for real-time security event processing at scale.

## Features

- **Complete Sigma Specification Support**: Full implementation of the Sigma rule language
- **High Performance**: Optimized pattern matching and event processing
- **Redpanda/Kafka Integration**: Built-in consumer with production-ready features
- **Async/Await**: Fully asynchronous design using Tokio
- **Comprehensive Error Handling**: Structured error types with detailed context
- **Production Ready**: Manual offset management, DLQ support, backpressure control
- **Extensive Testing**: Unit tests, integration tests, and property-based testing
- **Performance Benchmarks**: Comprehensive benchmark suite

## Quick Start

```rust
use sigma_rs::{DynamicEvent, rule};
use serde_json::json;

// Parse a Sigma rule
let rule = rule::rule_from_yaml(include_bytes!("rule.yml"))?;

// Create an event
let event = DynamicEvent::new(json!({
    "EventID": 1,
    "CommandLine": "powershell.exe -Command Get-Process"
}));

// Build the detection tree
let tree = sigma_rs::tree::Tree::from_rule(&rule).await?;

// Check if the event matches
let matches = tree.matches(&event).await?;
tracing::error!("Event matches: {}", matches.matched);
```

## Redpanda Integration

```rust
use sigma_rs::{SigmaEngineBuilder, KafkaConfig};

let kafka_config = KafkaConfig {
    brokers: "localhost:9092".to_string(),
    group_id: "sigma-processor".to_string(),
    topics: vec!["security-events".to_string()],
    ..Default::default()
};

let engine = SigmaEngineBuilder::new()
    .add_rule_dir("/path/to/rules")
    .with_kafka(kafka_config)
    .build()
    .await?;

engine.run().await?;
```

## Architecture

The library is organized into several key modules:

- **`rule`**: Sigma rule parsing and representation
- **`ast`**: Abstract syntax tree for rule conditions
- **`pattern`**: Pattern matching implementations
- **`tree`**: Detection tree construction and evaluation
- **`event`**: Event abstraction and field selection
- **`consumer`**: Redpanda/Kafka consumer implementation
- **`service`**: HTTP/gRPC service layer

## Consumer Features

- Manual offset management for reliability
- Dead letter queue (DLQ) support
- Exponential backoff retry logic
- Dynamic backpressure control
- Comprehensive metrics collection
- Graceful shutdown handling

## Performance

The implementation includes several optimizations:

- Lazy evaluation of rule conditions
- Efficient string matching with Aho-Corasick
- Memory-efficient event representation
- Parallel rule evaluation where possible
- Zero-copy string operations where possible

## Testing

Run the test suite:

```bash
# Run all tests
cargo test

# Run tests with coverage
cargo tarpaulin --out Html --output-dir coverage

# Run benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench simplified_benchmarks
```

## Examples

The `examples/` directory contains several usage examples:

- `rule_validator`: Validate Sigma rules in parallel
- `event_detector`: Process events from JSON files
- `stream_detector`: Real-time event stream processing
- `parallel_stream_detector`: Multi-threaded stream processing

## Configuration

The consumer supports extensive configuration:

```rust
let config = ConsumerConfig::builder()
    .brokers("localhost:9092")
    .group_id("sigma-consumer")
    .topics(vec!["events"])
    .batch_size(1000)
    .retry_policy(RetryPolicy {
        max_retries: 3,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(60),
        multiplier: 2.0,
    })
    .dlq_topic("dead-letter-events")
    .build();
```

## License

Apache 2.0

## Contributing

Contributions are welcome! Please see the [contributing guidelines](CONTRIBUTING.md) for details.
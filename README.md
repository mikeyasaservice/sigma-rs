# Sigma-rs: High-Performance Rust Implementation of Sigma Rules

[![CI](https://github.com/mikeyasaservice/sigma-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/mikeyasaservice/sigma-rs/actions/workflows/ci.yml)
[![Code Quality](https://github.com/mikeyasaservice/sigma-rs/actions/workflows/quality.yml/badge.svg)](https://github.com/mikeyasaservice/sigma-rs/actions/workflows/quality.yml)
[![Security](https://github.com/mikeyasaservice/sigma-rs/actions/workflows/security.yml/badge.svg)](https://github.com/mikeyasaservice/sigma-rs/actions/workflows/security.yml)
[![Coverage](https://codecov.io/gh/mikeyasaservice/sigma-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/mikeyasaservice/sigma-rs)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/sigma-rs.svg)](https://crates.io/crates/sigma-rs)
[![FOSSA Status](https://app.fossa.com/api/projects/custom%2B54172%2Fgithub.com%2Fseacurity%2Fsigma-rs.svg?type=shield&issueType=license)](https://app.fossa.com/projects/custom%2B54172%2Fgithub.com%2Fseacurity%2Fsigma-rs?ref=badge_shield&issueType=license)
[![FOSSA Status](https://app.fossa.com/api/projects/custom%2B54172%2Fgithub.com%2Fseacurity%2Fsigma-rs.svg?type=shield&issueType=security)](https://app.fossa.com/projects/custom%2B54172%2Fgithub.com%2Fseacurity%2Fsigma-rs?ref=badge_shield&issueType=security)

A production-ready Rust implementation of the Sigma rule engine with Redpanda/Kafka integration, designed for real-time security event processing at scale. Delivers **4.3x better performance** than existing Go implementations while maintaining full Sigma specification compatibility.

## Features

- **Complete Sigma Specification Support**: Full implementation of the Sigma rule language
- **Exceptional Performance**: 4.3x faster than Go implementations with 3M+ ops/sec throughput
- **Production-Ready Design**: Memory-safe, zero-cost abstractions, and predictable performance
- **Redpanda/Kafka Integration**: Built-in consumer with enterprise features
- **Async/Await Architecture**: Fully asynchronous design using Tokio for efficient concurrency
- **Comprehensive Error Handling**: Structured error types with detailed context
- **Enterprise Features**: Manual offset management, DLQ support, backpressure control
- **Extensive Testing**: Unit tests, integration tests, property-based testing, and performance benchmarks
- **Scientifically Validated**: Benchmarked against academic baselines with reproducible results

## Quick Start

### Using the CLI

```bash
# Clone and build
git clone https://github.com/mikeyasaservice/sigma-rs
cd sigma-rs
cargo build --release

# Run with the helper script
./run.sh --rules ./rules < events.json

# Or use make
make run < events.json

# Or run directly
./target/release/sigma-rs --rules ./rules < events.json
```

### Using as a Library

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
tracing::info!("Event matches: {}", matches.matched);
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

Sigma-rs delivers significant performance improvements over existing implementations through Rust's systems programming advantages and targeted optimizations.

### Benchmark Results

We conducted comprehensive benchmarks comparing sigma-rs against the original Go implementation from the academic study ["Towards implementing a streaming Sigma rule engine"](https://github.com/markus-kont/sigma-go) by Markus Kont and Mauno Pihelgas.

#### Test Environment
- **Hardware**: Apple M4 Pro (2024)
- **Rules**: 454 Windows Sigma rules from [SigmaHQ/sigma](https://github.com/SigmaHQ/sigma)
- **Test Data**: 1,000 realistic Windows events in ECS format
- **Total Operations**: 454,000 rule evaluations

#### Results Summary

| Metric | Go Study (Intel i7-8850H) | Sigma-rs (M4 Pro) | Improvement |
|--------|---------------------------|-------------------|-------------|
| **Per-operation** | 1,428 ns | 332 ns | **4.3x faster** |
| **Per-event (full ruleset)** | 670 μs | 151 μs | **4.4x faster** |
| **Throughput** | ~700K ops/sec | 3.0M ops/sec | **4.3x higher** |
| **Events/second** | ~1,490 | 6,617 | **4.4x higher** |

*Note: Different hardware architectures make direct comparison challenging, but the magnitude of improvement suggests significant algorithmic and implementation advantages.*

#### Detailed Performance Analysis

**Single Rule Evaluation:**
- Go implementation: 1,363-1,494 ns per operation
- Sigma-rs: 332 ns per operation
- Result: **4.3x performance improvement**

**Full Ruleset Processing:**
- Go study (469 rules): ~670 μs per event
- Sigma-rs (454 rules): 151 μs per event
- Normalized (469 rules): ~156 μs per event
- Result: **4.3x performance improvement**

**Memory Efficiency:**
- JSON decode overhead: 2.8 μs per event (1.9% of total)
- Rule processing: 151 μs per event (98.1% of total)
- Efficient memory layout minimizes allocation overhead

### Performance Optimizations

#### Core Engine Optimizations
- **Zero-cost abstractions**: Compile-time optimizations eliminate runtime overhead
- **Efficient pattern matching**: Aho-Corasick algorithms with optimized string handling
- **Lazy evaluation**: Rules short-circuit on first non-match to minimize work
- **Memory layout**: Cache-friendly data structures reduce memory access latency

#### String Processing
- **Zero-copy operations**: Minimize string allocations and copies
- **Interned strings**: Reduce memory footprint for repeated field names
- **SIMD-optimized matching**: Leverage hardware acceleration where available

#### Async Performance
- **Tokio runtime**: Efficient async task scheduling
- **Minimal context switching**: Reduced overhead compared to traditional threading
- **Backpressure handling**: Prevents memory exhaustion under load

#### Rule Compilation
- **AST optimization**: Efficient tree structures for rule evaluation
- **Pattern compilation**: Pre-compiled regex and glob patterns
- **Field indexing**: Fast field lookup in event data

### Scalability Characteristics

**Throughput Scaling:**
- Linear scaling with CPU cores for independent events
- Efficient memory usage prevents GC pressure
- Predictable performance under sustained load

**Rule Set Scaling:**
- O(n) complexity for rule evaluation per event
- Memory usage scales linearly with rule count
- No significant performance degradation with large rule sets

**Event Complexity:**
- Performance independent of event size for field-based matching
- Efficient field selection minimizes parsing overhead
- Regex patterns may show complexity-dependent performance

### Production Performance

**Real-world Deployment Characteristics:**
- Sustained throughput: 6,000+ events/second (454 rules)
- Memory usage: <100MB for 500 rules + event buffers
- CPU utilization: Efficient multi-core usage
- Latency: Sub-millisecond rule evaluation

**Kafka Consumer Performance:**
- Batch processing: 1,000+ events per batch
- Offset management: Minimal overhead
- Backpressure: Automatic throttling under load
- Error handling: DLQ processing with minimal impact

### Benchmark Reproduction

Run performance benchmarks locally:

```bash
# Generate test data
cargo run --bin data_generator --release -- --count 1000 --output events.jsonl

# Run Go study comparison
cargo run --example go_study_comparison --release -- --count 1000

# Run comprehensive benchmarks
cargo bench

# Profile with detailed metrics
cargo run --example comprehensive_benchmark --release -- --count 10000
```

The benchmarks use realistic Windows event data and actual Sigma rules from the community repository, ensuring results reflect real-world performance.

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
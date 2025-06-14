# Performance Guide

## Benchmarking

Run the benchmark suite to establish baselines for your hardware:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench pattern_matching

# Quick benchmarks for CI
cargo bench -- --quick
```

## Performance Optimization Tips

### 1. Rule Organization

Group similar rules to maximize pattern matching efficiency:

```yaml
# Good: Similar patterns grouped
detection:
  suspicious_powershell:
    CommandLine|contains:
      - 'powershell.exe'
      - 'pwsh.exe'
      - '-enc'
      - '-EncodedCommand'
```

### 2. String Interning

For deployments with many similar patterns:

```rust
use sigma_rs::pattern::intern::{set_interner_config, InternerConfig};

// Enable at startup
set_interner_config(InternerConfig {
    enabled: true,
    max_size: 10_000,  // Adjust based on rule count
});
```

### 3. Parallel Processing

```rust
// Use all available cores
let engine = SigmaEngineBuilder::new()
    .with_worker_threads(num_cpus::get())
    .build()
    .await?;

// Or set specific count
.with_worker_threads(8)
```

### 4. Kafka Tuning

```rust
let kafka_config = KafkaConfig {
    // Larger batches = better throughput
    batch_size: 5000,
    
    // More parallel consumers
    parallel_consumers: 4,
    
    // Tune based on event size
    buffer_size: 50_000,
    
    // Adjust for latency vs throughput
    max_wait_time: Duration::from_millis(100),
};
```

### 5. Memory Optimization

```rust
// For memory-constrained environments
let engine = SigmaEngineBuilder::new()
    .with_max_rules(1000)  // Limit rule count
    .with_worker_threads(2)  // Reduce parallelism
    .build()
    .await?;
```

## Performance Metrics

### Expected Performance

On modern hardware (8-core CPU, 16GB RAM):

- **Rule Loading**: ~1,000 rules/second
- **Simple Rules**: 500,000+ events/second
- **Complex Rules**: 100,000+ events/second
- **Memory Usage**: ~200MB base + 100KB per rule

### Monitoring

Enable metrics endpoint:

```rust
// Prometheus metrics
GET /metrics

# Key metrics to monitor:
- sigma_rules_loaded_total
- sigma_events_processed_total
- sigma_evaluation_duration_seconds
- sigma_kafka_lag
- sigma_memory_usage_bytes
```

### Profiling

```bash
# CPU profiling
cargo build --release --features profiling
perf record --call-graph=dwarf ./target/release/sigma-rs
perf report

# Memory profiling
valgrind --tool=massif ./target/release/sigma-rs
ms_print massif.out.*
```

## Common Performance Issues

### High CPU Usage

1. Check for complex regex patterns
2. Reduce worker thread count
3. Enable string interning

### High Memory Usage

1. Enable string interning
2. Reduce rule count per directory
3. Lower Kafka batch sizes

### Slow Rule Loading

1. Check for oversized rule files
2. Validate YAML syntax
3. Reduce directory depth

### Kafka Lag

1. Increase parallel consumers
2. Increase batch size
3. Add more worker threads

## Best Practices

1. **Start Simple**: Begin with default settings
2. **Measure First**: Use benchmarks to establish baselines
3. **Tune Gradually**: Change one parameter at a time
4. **Monitor Production**: Use metrics to guide optimization
5. **Profile Hotspots**: Focus on actual bottlenecks

## Example Configurations

### High Throughput
```rust
SigmaEngineBuilder::new()
    .with_worker_threads(16)
    .with_kafka(KafkaConfig {
        batch_size: 10_000,
        parallel_consumers: 8,
        ..Default::default()
    })
```

### Low Latency
```rust
SigmaEngineBuilder::new()
    .with_worker_threads(4)
    .with_kafka(KafkaConfig {
        batch_size: 100,
        max_wait_time: Duration::from_millis(10),
        ..Default::default()
    })
```

### Memory Constrained
```rust
SigmaEngineBuilder::new()
    .with_worker_threads(2)
    .with_string_interning(true)
    .with_max_rules(500)
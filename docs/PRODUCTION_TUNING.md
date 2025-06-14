# Production Tuning Guide

This guide provides detailed tuning recommendations for deploying sigma-rs in production environments.

## Table of Contents

1. [Performance Baselines](#performance-baselines)
2. [Resource Planning](#resource-planning)
3. [Kafka Tuning](#kafka-tuning)
4. [Memory Optimization](#memory-optimization)
5. [CPU Optimization](#cpu-optimization)
6. [Network Optimization](#network-optimization)
7. [Monitoring & Alerting](#monitoring--alerting)
8. [Troubleshooting](#troubleshooting)

## Performance Baselines

### Hardware Recommendations

| Workload | CPU | Memory | Network | Storage |
|----------|-----|---------|---------|----------|
| Small (< 1K events/sec) | 2-4 cores | 2-4 GB | 1 Gbps | 50 GB SSD |
| Medium (1K-10K events/sec) | 4-8 cores | 8-16 GB | 10 Gbps | 100 GB SSD |
| Large (10K-100K events/sec) | 16-32 cores | 32-64 GB | 10 Gbps | 500 GB SSD |
| XLarge (> 100K events/sec) | 32+ cores | 64+ GB | 25 Gbps | 1+ TB NVMe |

### Expected Performance

With proper tuning, expect:
- **Latency**: p50 < 1ms, p95 < 5ms, p99 < 10ms
- **Throughput**: 10-50K events/sec per core (depending on rule complexity)
- **Memory**: ~200MB base + 100KB per rule + event buffer

## Resource Planning

### CPU Allocation

```yaml
# Kubernetes resource requests/limits
resources:
  requests:
    cpu: "4"      # Baseline
  limits:
    cpu: "8"      # Allow bursting

# Worker threads configuration
SIGMA_WORKER_THREADS: 6  # Less than CPU limit to leave room for OS
```

### Memory Allocation

```yaml
resources:
  requests:
    memory: "4Gi"
  limits:
    memory: "8Gi"

# JVM-style heap settings for Rust
RUST_MIN_STACK: "2097152"  # 2MB stack size
```

### Formula for Memory Planning

```
Total Memory = Base (200MB)
             + (Number of Rules × 100KB)
             + (Kafka Buffer Size × Event Size)
             + (Worker Threads × 50MB)
             + 20% overhead
```

## Kafka Tuning

### Consumer Configuration

```rust
KafkaConfig {
    // Batch processing for throughput
    batch_size: 5000,              // Increase for higher throughput
    max_wait_time: Duration::from_millis(100), // Decrease for lower latency
    
    // Parallelism
    parallel_consumers: 8,         // Match number of partitions
    
    // Buffer management
    buffer_size: 50_000,          // In-memory buffer
    
    // Compression
    compression: "lz4",           // Best balance of speed/ratio
    
    // Network tuning
    socket_receive_buffer_bytes: 2 * 1024 * 1024,  // 2MB
    socket_send_buffer_bytes: 2 * 1024 * 1024,     // 2MB
    
    // Session management
    session_timeout_ms: 30_000,
    heartbeat_interval_ms: 3_000,
}
```

### Partition Strategy

```bash
# Calculate optimal partitions
Partitions = max(
    Expected Throughput (MB/s) / 10,  # 10 MB/s per partition
    Number of Consumers × 2           # Allow for scaling
)

# Example: 100 MB/s throughput, 8 consumers
Partitions = max(100/10, 8×2) = 16 partitions
```

### Backpressure Configuration

```rust
// Adaptive backpressure
BackpressureConfig {
    initial_capacity: 1000,
    max_capacity: 10_000,
    pressure_threshold: 0.8,      // Start applying at 80% full
    release_threshold: 0.6,       // Release at 60% full
}
```

## Memory Optimization

### String Interning

```rust
// Enable for deployments with many similar patterns
use sigma_rs::pattern::intern::{set_interner_config, InternerConfig};

set_interner_config(InternerConfig {
    enabled: true,
    max_size: 50_000,           // Increase for more rules
    eviction_policy: "lru",     // Least recently used
    ttl: Duration::from_secs(3600), // 1 hour TTL
});
```

### Rule Loading Strategy

```rust
// Lazy loading for large rule sets
RuleSetConfig {
    lazy_loading: true,
    max_rules_in_memory: 10_000,
    cache_size: 5_000,
    preload_common_rules: true,
}
```

### Memory Profiling

```bash
# Enable jemalloc for better memory profiling
export MALLOC_CONF="prof:true,prof_prefix:sigma-rs"

# Run with memory tracking
./sigma-rs --memory-profile

# Analyze with jeprof
jeprof --show_bytes --pdf sigma-rs jeprof.*.heap > memory.pdf
```

## CPU Optimization

### Thread Pool Tuning

```rust
// CPU-bound workload optimization
let cpu_threads = num_cpus::get();
let worker_threads = (cpu_threads as f32 * 0.75) as usize; // Leave 25% for OS

SigmaEngineBuilder::new()
    .with_worker_threads(worker_threads)
    .with_blocking_threads(cpu_threads / 4) // For I/O
```

### NUMA Awareness

```yaml
# Kubernetes NUMA binding
spec:
  containers:
  - name: sigma-rs
    resources:
      requests:
        cpu: "8"
        memory: "16Gi"
    # NUMA topology manager
    topologySpreadConstraints:
    - maxSkew: 1
      topologyKey: topology.kubernetes.io/zone
      whenUnsatisfiable: DoNotSchedule
```

### CPU Profiling

```bash
# Profile CPU usage
perf record -F 99 -g ./sigma-rs
perf report

# Flame graph generation
cargo install flamegraph
cargo flamegraph --bench rule_matching
```

## Network Optimization

### TCP Tuning

```bash
# System-level TCP optimization
sysctl -w net.core.rmem_max=134217728
sysctl -w net.core.wmem_max=134217728
sysctl -w net.ipv4.tcp_rmem="4096 87380 134217728"
sysctl -w net.ipv4.tcp_wmem="4096 65536 134217728"
sysctl -w net.core.netdev_max_backlog=5000
```

### Connection Pooling

```rust
// HTTP client configuration
HttpClientConfig {
    pool_idle_timeout: Duration::from_secs(60),
    pool_max_idle_per_host: 100,
    tcp_keepalive: Some(Duration::from_secs(60)),
    tcp_nodelay: true,
    http2_keep_alive_interval: Some(Duration::from_secs(30)),
}
```

## Monitoring & Alerting

### Key Metrics to Monitor

1. **Application Metrics**
   - `sigma_events_processed_total` - Event throughput
   - `sigma_rule_evaluation_duration_seconds` - Processing latency
   - `sigma_matches_total` - Detection rate
   - `sigma_errors_total` - Error rate

2. **Kafka Metrics**
   - `sigma_kafka_consumer_lag` - Consumer lag
   - `sigma_kafka_offset_commit_duration` - Commit latency
   - `sigma_kafka_rebalance_total` - Rebalance frequency

3. **System Metrics**
   - CPU utilization by core
   - Memory usage (RSS, heap, stack)
   - Network I/O (bytes/packets)
   - Disk I/O (if using persistent rules)

### Grafana Dashboard

```json
{
  "dashboard": {
    "title": "Sigma-rs Production Metrics",
    "panels": [
      {
        "title": "Event Processing Rate",
        "targets": [{
          "expr": "rate(sigma_events_processed_total[5m])"
        }]
      },
      {
        "title": "Rule Evaluation Latency",
        "targets": [{
          "expr": "histogram_quantile(0.99, sigma_rule_evaluation_duration_seconds)"
        }]
      },
      {
        "title": "Kafka Consumer Lag",
        "targets": [{
          "expr": "sigma_kafka_consumer_lag"
        }]
      }
    ]
  }
}
```

### Alert Rules

```yaml
groups:
- name: sigma-rs
  rules:
  - alert: HighConsumerLag
    expr: sigma_kafka_consumer_lag > 100000
    for: 5m
    annotations:
      summary: "High Kafka consumer lag detected"
      
  - alert: HighErrorRate
    expr: rate(sigma_errors_total[5m]) > 0.01
    for: 5m
    annotations:
      summary: "Error rate above 1%"
      
  - alert: HighMemoryUsage
    expr: container_memory_usage_bytes / container_spec_memory_limit_bytes > 0.9
    for: 5m
    annotations:
      summary: "Memory usage above 90%"
```

## Troubleshooting

### High CPU Usage

1. **Check rule complexity**
   ```bash
   sigma-rs analyze-rules --complexity /path/to/rules
   ```

2. **Profile hot paths**
   ```bash
   CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release
   perf top -p $(pgrep sigma-rs)
   ```

3. **Reduce worker threads**
   ```bash
   SIGMA_WORKER_THREADS=4 ./sigma-rs
   ```

### High Memory Usage

1. **Enable string interning**
   ```rust
   SIGMA_STRING_INTERNING_ENABLED=true
   SIGMA_STRING_INTERNING_MAX_SIZE=100000
   ```

2. **Reduce batch sizes**
   ```rust
   KAFKA_BATCH_SIZE=1000
   KAFKA_BUFFER_SIZE=10000
   ```

3. **Check for memory leaks**
   ```bash
   valgrind --leak-check=full --show-leak-kinds=all ./sigma-rs
   ```

### Kafka Lag

1. **Increase parallelism**
   ```yaml
   kafka:
     parallel_consumers: 16  # Increase consumers
   ```

2. **Optimize batch processing**
   ```yaml
   kafka:
     batch_size: 10000      # Larger batches
     max_wait_time: 200ms   # Higher latency tolerance
   ```

3. **Check network bandwidth**
   ```bash
   iftop -i eth0
   ```

### Performance Degradation

1. **Check GC pressure (if using jemalloc)**
   ```bash
   MALLOC_CONF="stats_print:true" ./sigma-rs
   ```

2. **Review rule changes**
   ```bash
   git diff --name-only | grep -E "\.ya?ml$" | xargs sigma-rs validate --timing
   ```

3. **Enable detailed tracing**
   ```bash
   RUST_LOG=sigma_rs=trace,sigma_rs::consumer=trace ./sigma-rs
   ```

## Best Practices

1. **Start Conservative**: Begin with lower throughput targets and scale up
2. **Monitor Continuously**: Use metrics to guide optimization
3. **Test Changes**: Benchmark before and after tuning
4. **Document Settings**: Keep track of what works for your workload
5. **Plan for Peaks**: Size for 2x average load
6. **Regular Maintenance**: Update rules and restart periodically

## Example Production Configurations

### High Throughput Setup
```yaml
# 100K+ events/sec
sigma:
  worker_threads: 32
  string_interning:
    enabled: true
    max_size: 100000
kafka:
  batch_size: 10000
  buffer_size: 100000
  parallel_consumers: 16
  compression: "lz4"
```

### Low Latency Setup
```yaml
# < 5ms p99 latency
sigma:
  worker_threads: 8
  string_interning:
    enabled: false  # Reduce overhead
kafka:
  batch_size: 100
  buffer_size: 1000
  parallel_consumers: 4
  max_wait_time: 10ms
```

### Balanced Setup
```yaml
# Good default for most workloads
sigma:
  worker_threads: 16
  string_interning:
    enabled: true
    max_size: 50000
kafka:
  batch_size: 1000
  buffer_size: 10000
  parallel_consumers: 8
  max_wait_time: 100ms
```
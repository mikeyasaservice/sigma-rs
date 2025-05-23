# Redpanda Consumer Implementation Plan

## Overview
This document outlines the plan to enhance the existing Kafka consumer implementation in the Rust Sigma engine to make it production-ready with robust error handling, metrics, and operational features.

## Architecture

### Module Structure
```
src/consumer/
├── mod.rs              # Public API and module exports
├── config.rs           # Configuration structures
├── consumer.rs         # Core consumer implementation
├── processor.rs        # Message processing abstraction
├── error.rs            # Consumer-specific error types
├── metrics.rs          # Metrics collection
├── offset_manager.rs   # Manual offset management
└── backpressure.rs     # Backpressure control
```

### Key Components

1. **RedpandaConsumer**: Main consumer struct with lifecycle management
2. **MessageProcessor**: Trait for processing messages with custom logic
3. **OffsetManager**: Handles manual offset commits and state
4. **ConsumerMetrics**: Collects operational metrics
5. **BackpressureController**: Manages flow control

## Implementation Phases

### Phase 1: Core Infrastructure
1. ☐ Extract consumer into dedicated module
2. ☐ Create configuration structures
3. ☐ Implement MessageProcessor trait
4. ☐ Add structured error types
5. ☐ Implement manual offset management

### Phase 2: Reliability & Error Handling
1. ☐ Add retry logic with exponential backoff
2. ☐ Implement dead letter queue support
3. ☐ Add comprehensive error recovery
4. ☐ Handle consumer rebalancing
5. ☐ Implement transaction support (optional)

### Phase 3: Observability
1. ☐ Add metrics collection (lag, throughput, errors)
2. ☐ Implement health checks
3. ☐ Add detailed logging
4. ☐ Create dashboard templates
5. ☐ Add distributed tracing

### Phase 4: Performance & Operations
1. ☐ Implement backpressure control
2. ☐ Add batching support
3. ☐ Optimize memory usage
4. ☐ Add connection pooling
5. ☐ Implement graceful shutdown

### Phase 5: Testing & Documentation
1. ☐ Unit tests for all components
2. ☐ Integration tests with testcontainers
3. ☐ Performance benchmarks
4. ☐ Chaos testing scenarios
5. ☐ Complete documentation

## Implementation Details

### Message Processor
```rust
#[async_trait]
pub trait MessageProcessor: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync;
    
    async fn process(&self, message: BorrowedMessage<'_>) -> Result<(), Self::Error>;
    async fn on_success(&self, message: &BorrowedMessage<'_>);
    async fn on_failure(&self, error: &Self::Error, message: &BorrowedMessage<'_>);
}
```

### Consumer Configuration
```rust
pub struct ConsumerConfig {
    // Kafka settings
    pub brokers: String,
    pub group_id: String,
    pub topics: Vec<String>,
    pub session_timeout_ms: u32,
    
    // Processing settings
    pub batch_size: usize,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    
    // Backpressure settings
    pub channel_buffer_size: usize,
    pub max_inflight_messages: usize,
    
    // DLQ settings
    pub dlq_topic: Option<String>,
    
    // Metrics settings
    pub metrics_interval_secs: u64,
}
```

### Error Handling Strategy
1. Transient errors: Retry with backoff
2. Parsing errors: Send to DLQ
3. Fatal errors: Log and continue
4. Connection errors: Reconnect automatically

### Metrics to Collect
- Messages consumed per second
- Messages processed per second
- Processing latency (p50, p95, p99)
- Consumer lag by partition
- Error rates by type
- DLQ message count
- Rebalance events

### Graceful Shutdown Sequence
1. Stop accepting new messages
2. Process in-flight messages
3. Commit final offsets
4. Close connections
5. Flush metrics

## Testing Strategy

### Unit Tests
- Test each component in isolation
- Mock external dependencies
- Test error scenarios

### Integration Tests
- Use testcontainers for Kafka
- Test full message flow
- Test failure scenarios
- Test rebalancing

### Performance Tests
- Benchmark throughput
- Test backpressure
- Memory usage analysis
- Latency measurements

## Migration Plan
1. Implement new consumer alongside existing
2. Add feature flag for switching
3. Test in staging environment
4. Gradual rollout in production
5. Remove old implementation

## Success Criteria
- Zero message loss
- < 100ms p99 processing latency
- Automatic recovery from failures
- Clear operational metrics
- Comprehensive test coverage
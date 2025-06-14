# Distributed Arrow-Based Sigma-rs Implementation Plan

## Overview
Transform sigma-rs from single-node event-at-a-time processing to distributed columnar processing using Apache Arrow, achieving 2-5M events/sec per node and 100M+ events/sec distributed.

## Goals
1. **Single-node performance**: 2-5M events/sec using Arrow columnar processing
2. **Distributed performance**: 100M+ events/sec with linear scaling
3. **Single binary**: One executable that can run as coordinator, worker, or standalone
4. **Zero external dependencies**: Embedded consensus, no ZooKeeper/Kafka required
5. **Drop-in replacement**: Backward compatible with existing deployments

## Architecture

### Component Overview
```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Arrow Input   │────▶│  Rule Compiler  │────▶│ Arrow Evaluator │
│  (RecordBatch)  │     │  (DataFusion)   │     │    (SIMD)       │
└─────────────────┘     └─────────────────┘     └─────────────────┘
         │                                                │
         │                                                ▼
         │                                       ┌─────────────────┐
         └──────────────────────────────────────▶│  Arrow Output   │
                                                 │  (RecordBatch)  │
                                                 └─────────────────┘
```

### Distributed Mode
```
                    ┌──────────────┐
                    │ Coordinator  │ (Raft consensus)
                    │ (Port 8080)  │
                    └──────┬───────┘
                           │ Arrow Flight RPC
        ┌──────────────────┼──────────────────┐
        │                  │                  │
   ┌────▼────┐       ┌────▼────┐       ┌────▼────┐
   │ Worker  │       │ Worker  │       │ Worker  │
   │  Node   │       │  Node   │       │  Node   │
   └─────────┘       └─────────┘       └─────────┘
```

## Implementation Phases

### Phase 1: Arrow Foundation (Week 1-2)

#### 1.1 Add Dependencies
```toml
[dependencies]
arrow = { version = "52.0", features = ["ipc", "compute"] }
arrow-json = "52.0"
arrow-flight = "52.0"
datafusion = "40.0"
datafusion-expr = "40.0"
parquet = { version = "52.0", optional = true }
```

#### 1.2 Create Arrow Event Model
- `src/event/arrow_event.rs` - RecordBatch-based event representation
- `src/event/json_to_arrow.rs` - Streaming JSON to Arrow converter
- Schema inference from Sigma rules

#### 1.3 Implement Columnar Pattern Matching
- `src/pattern/arrow_string_matcher.rs` - SIMD string operations
- `src/pattern/arrow_numeric_matcher.rs` - Vectorized numeric comparisons
- Integrate with existing pattern module

### Phase 2: DataFusion Integration (Week 2-3)

#### 2.1 Rule Compilation
- `src/rule/datafusion_compiler.rs` - Convert Sigma rules to DataFusion expressions
- Support for all Sigma operators (contains, startswith, endswith, regex, etc.)
- Optimize common patterns

#### 2.2 Batch Evaluation Engine
- `src/engine/arrow_engine.rs` - Main Arrow processing engine
- `src/ruleset/arrow_ruleset.rs` - Batch rule evaluation
- Maintain backward compatibility with existing API

#### 2.3 Performance Optimizations
- Predicate pushdown
- Column pruning
- Expression optimization

### Phase 3: Distributed Runtime (Week 3-4)

#### 3.1 Networking Layer
- `src/distributed/coordinator.rs` - Raft-based coordinator
- `src/distributed/worker.rs` - Worker node implementation
- `src/distributed/flight_service.rs` - Arrow Flight RPC service

#### 3.2 Work Distribution
- Hash-based partitioning by configurable key
- Dynamic load balancing
- Backpressure propagation

#### 3.3 Operational Features
- Health checks and monitoring
- Graceful shutdown
- Configuration management

### Phase 4: Integration & Polish (Week 4-5)

#### 4.1 CLI Enhancements
```bash
# Standalone mode (backward compatible)
sigma-rs --rules ./rules < events.json

# Distributed coordinator
sigma-rs --mode coordinator --bind 0.0.0.0:8080 --rules s3://rules/

# Distributed worker
sigma-rs --mode worker --coordinator coordinator:8080

# Auto mode with discovery
sigma-rs --mode auto --cluster-name prod --rules s3://rules/
```

#### 4.2 Streaming Input/Output
- Arrow IPC format support
- Parquet file support
- Kafka integration with Arrow batching

#### 4.3 Production Hardening
- Comprehensive error handling
- Performance profiling
- Documentation and examples

## Technical Details

### Arrow Schema Design
```rust
// Dynamic schema based on rule requirements
struct SigmaArrowSchema {
    // Core fields always included
    timestamp: DataType::Timestamp(TimeUnit::Microsecond, None),
    host: DataType::Utf8,
    
    // Dynamic fields based on rules
    rule_fields: HashMap<String, DataType>,
}
```

### Columnar Rule Evaluation
```rust
// Before (row-based)
for event in events {
    for rule in rules {
        if rule.matches(&event) { ... }
    }
}

// After (columnar)
let batch: RecordBatch = events.into();
let filter_mask = rule.to_datafusion_expr().evaluate(&batch)?;
let matches = batch.filter(&filter_mask)?;
```

### Distribution Strategy
- **Partitioning**: By host/source for locality
- **Replication**: Rules replicated to all workers
- **Routing**: Consistent hashing with virtual nodes
- **Failover**: Automatic work redistribution

## Performance Targets

### Single Node
- Input: 2-5M events/sec
- Latency: < 10ms p99
- Memory: < 1GB for 10k rules
- CPU: 80% utilization on 16 cores

### Distributed (20 nodes)
- Input: 40-100M events/sec
- Latency: < 50ms p99
- Scaling: Linear up to 100 nodes
- Fault tolerance: 1-2 node failures

## Migration Path

1. **v1.1.0**: Arrow support in experimental mode
2. **v1.2.0**: Arrow as default, JSON as fallback
3. **v1.3.0**: Distributed mode (beta)
4. **v2.0.0**: Distributed as primary mode

## Risk Mitigation

### Technical Risks
- **Arrow API changes**: Pin versions, comprehensive tests
- **Memory usage**: Implement batch size limits
- **Network partitions**: Raft handles split-brain

### Operational Risks
- **Backward compatibility**: Maintain JSON pipeline
- **Deployment complexity**: Excellent documentation
- **Performance regression**: Comprehensive benchmarks

## Success Criteria

1. Single-node: 2M+ events/sec on reference hardware
2. Distributed: 100M+ events/sec on 20-node cluster
3. Drop-in replacement for existing deployments
4. Fortune 20 company successful POC

## Timeline

- Week 1-2: Arrow foundation
- Week 2-3: DataFusion integration
- Week 3-4: Distributed runtime
- Week 4-5: Integration & polish
- Week 6: Testing & documentation

Total: 6 weeks to production-ready distributed sigma-rs
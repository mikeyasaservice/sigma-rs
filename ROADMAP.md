# Sigma-rs Roadmap

## Vision
Transform sigma-rs into the industry's fastest distributed security event processing engine, capable of handling 100M+ events/second while maintaining the simplicity of a single binary deployment.

## Current State (v1.0.2)
- ✅ Single-node processing: ~75k events/second
- ✅ 2,302 Sigma rules support
- ✅ Kafka integration
- ✅ Production-ready with < 10MB memory footprint
- ✅ Comprehensive test coverage

## Roadmap Overview

### Phase 1: Arrow Foundation with Flight Server (v1.1.0) - Q1 2025
**Goal**: Single binary achieving 2-5M events/second with built-in Arrow Flight server

#### Single Binary Architecture
```bash
# All capabilities in ONE binary
sigma-rs [OPTIONS]
  --rules <path>              # Sigma rules directory
  --input <json|kafka|flight> # Input source
  --output <json|kafka|flight> # Output destination  
  --flight-server <addr>      # Enable Arrow Flight server
  --mode <standalone|coordinator|worker> # Deployment mode
```

#### Tokio + Arrow Integration
- **Keep Tokio for**: I/O, networking, service layer, worker orchestration
- **Add Arrow for**: Columnar data processing, SIMD operations, batch evaluation
- **Add Flight for**: Zero-copy streaming protocol, built into main binary

#### Key Features
- [ ] Arrow-based event model alongside JSON
- [ ] Streaming JSON to Arrow RecordBatch converter
- [ ] Columnar pattern matching with SIMD optimizations
- [ ] **Built-in Arrow Flight server** (no separate process)
- [ ] Flight client for upstream consumption
- [ ] Benchmark: 2M+ events/second single node

#### Technical Details
```rust
// Tokio handles async I/O and orchestration
tokio::spawn(async {
    // Arrow handles batch processing
    let batch: RecordBatch = json_stream.collect_batch(10_000).await?;
    let matches = ruleset.evaluate_arrow(batch).await?;
});
```

### Phase 2: DataFusion Integration (v1.2.0) - Q1 2025
**Goal**: Compile Sigma rules to optimized query plans

#### Features
- [ ] Sigma rule to DataFusion expression compiler
- [ ] Query optimization (predicate pushdown, column pruning)
- [ ] Complex aggregations using DataFusion windows
- [ ] Maintain backward compatibility with JSON pipeline

#### Performance Targets
- Single-node: 5M events/second
- Memory: < 1GB for 10k rules
- Latency: < 10ms p99

### Phase 3: Distributed Runtime (v1.3.0) - Q2 2025
**Goal**: Linear scaling to 100M+ events/second with single binary

#### Architecture
```
Coordinator (Raft consensus, Arrow Flight server)
     ├── Worker 1 (2-5M events/sec, Arrow Flight server)
     ├── Worker 2 (2-5M events/sec, Arrow Flight server)
     └── Worker N (2-5M events/sec, Arrow Flight server)

All nodes run the SAME binary with different --mode flags
```

#### Features
- [ ] Single binary with multiple modes (coordinator/worker/auto)
- [ ] Arrow Flight for data plane (built-in, not separate service)
- [ ] Embedded Raft consensus (no external dependencies)
- [ ] Consistent hashing for event routing
- [ ] Automatic failover and rebalancing
- [ ] Direct worker-to-worker Flight communication

#### Tokio's Role in Distributed Mode
- **gRPC service layer**: Using existing tonic integration
- **Worker pool management**: Leverage existing patterns
- **Channel-based coordination**: mpsc for work distribution
- **Async Arrow Flight**: Streaming with backpressure

### Phase 4: Enterprise Features (v2.0.0) - Q3 2025
**Goal**: Production deployment at Fortune 20 scale

#### Features
- [ ] Multi-region support with geo-routing
- [ ] Hot rule reloading without downtime
- [ ] Advanced monitoring and observability
- [ ] Integration with major SIEM platforms
- [ ] Kubernetes operator for automated deployment

#### Performance at Scale
- 100 nodes: 200M+ events/second
- Sub-second rule updates globally
- 99.99% availability

## Technical Evolution

### Current Architecture (Tokio-based)
```
Kafka → Tokio Streams → JSON Parse → Rule Evaluation → Output
         (async I/O)    (per event)   (parallel tasks)
```

### Future Architecture (Tokio + Arrow)
```
Kafka → Tokio Streams → Arrow Batch → DataFusion → Arrow Output → JSON
         (async I/O)     (columnar)    (vectorized)  (zero-copy)
```

### Why Tokio + Arrow is Powerful
1. **Tokio excels at**:
   - Async I/O and networking
   - Concurrent task orchestration
   - Service layer (HTTP/gRPC)
   - Graceful shutdown and lifecycle

2. **Arrow/DataFusion excels at**:
   - Columnar data processing
   - SIMD vectorization
   - Query optimization
   - Zero-copy operations

3. **Together they provide**:
   - High-throughput I/O with efficient processing
   - Low-latency streaming with batch efficiency
   - Distributed coordination with local optimization

## Migration Path

### For Current Users
1. **v1.0.x → v1.1.0**: No changes required, Arrow disabled by default
2. **v1.1.0 → v1.2.0**: Enable Arrow with `--engine arrow` flag
3. **v1.2.0 → v1.3.0**: Distributed mode opt-in with `--mode`
4. **v1.3.0 → v2.0.0**: Distributed by default, standalone available

### Backward Compatibility
- JSON pipeline maintained through v2.0
- Existing Kafka integration enhanced, not replaced
- CLI interface remains stable
- Configuration format unchanged

## Success Metrics

### Performance
- **v1.1.0**: 2M events/sec (26x improvement)
- **v1.2.0**: 5M events/sec (66x improvement)
- **v1.3.0**: 100M events/sec distributed (1,333x improvement)

### Adoption
- **v1.1.0**: 10 enterprise POCs
- **v1.2.0**: 3 Fortune 500 deployments
- **v2.0.0**: 20 Fortune 500 deployments

### Community
- 1,000+ GitHub stars
- 50+ contributors
- Sigma rule marketplace

## Development Timeline

| Version | Target Date | Key Features | Performance |
|---------|------------|--------------|-------------|
| v1.0.2  | Done | Current release | 75k/sec |
| v1.1.0  | Feb 2025 | Arrow foundation | 2M/sec |
| v1.2.0  | Mar 2025 | DataFusion | 5M/sec |
| v1.3.0  | May 2025 | Distributed | 100M/sec |
| v2.0.0  | Aug 2025 | Enterprise | 200M+/sec |

## Getting Involved

### Contributing
- Performance improvements
- Sigma rule compatibility
- Documentation and examples
- Integration connectors

### Testing
- Early access program for v1.1.0
- Distributed testing for v1.3.0
- Production feedback

### Community
- Discord: [coming soon]
- Monthly community calls
- SigmaCon 2025 presentation

---

*This roadmap is subject to change based on community feedback and production requirements.*
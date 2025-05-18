# Comprehensive Rust Conversion Plan for Go Sigma Rule Engine

## Executive Summary

This document outlines a detailed plan for converting the Go-based Sigma rule engine to Rust, with specific focus on Redpanda integration, performance optimization, and memory safety improvements.

## 1. Architecture Analysis

### Current Go Architecture

1. **Core Interfaces**:
   - `Event`: Combines `Keyworder` and `Selector` interfaces
   - `Matcher`: Core matching interface for AST nodes
   - `Branch`: Enhanced Matcher for tree traversal

2. **Key Components**:
   - **Lexer**: Token-based parsing with channels for communication
   - **Parser**: Builds AST from tokens
   - **Tree**: Represents complete AST for rule evaluation
   - **Pattern Matching**: String/numeric matchers with various strategies
   - **Rule Management**: YAML parsing and ruleset organization
   - **Error Handling**: Structured error types with context

3. **Concurrency**: Uses goroutines and channels for lexer/parser communication

## 2. Rust Architecture Design

### Rust Equivalents

1. **Go → Rust Pattern Mapping**:
   ```rust
   // Go interface → Rust trait
   pub trait Event: Keyworder + Selector {}
   
   // Go struct embedding → Rust composition
   pub struct Tree {
       root: Box<dyn Branch>,
       rule: Option<RuleHandle>,
   }
   
   // Go channels → Rust async channels or crossbeam
   use tokio::sync::mpsc;
   use crossbeam::channel;
   
   // Go goroutines → Tokio tasks or native threads
   use tokio::task;
   ```

2. **Core Traits**:
   ```rust
   pub trait Keyworder {
       fn keywords(&self) -> (Vec<String>, bool);
   }
   
   pub trait Selector {
       fn select(&self, key: &str) -> (Option<Value>, bool);
   }
   
   pub trait Matcher {
       fn matches(&self, event: &dyn Event) -> (bool, bool);
   }
   
   pub trait Branch: Matcher {
       fn as_any(&self) -> &dyn Any;
   }
   ```

## 3. Phased Implementation Plan

### Phase 1: Core Data Structures and Types (Week 1-2)

**Goal**: Establish foundational types and traits

**Dependencies**:
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
regex = "1.10"
glob = "0.3"
thiserror = "1.0"
anyhow = "1.0"
```

**Tasks**:
1. Define core traits (`Event`, `Matcher`, `Branch`)
2. Implement token types and lexer structures
3. Create error types using `thiserror`
4. Set up basic project structure:
   ```
   src/
   ├── lib.rs
   ├── event.rs
   ├── matcher.rs
   ├── lexer/
   │   ├── mod.rs
   │   └── token.rs
   ├── parser/
   │   ├── mod.rs
   │   └── ast.rs
   ├── rule/
   │   ├── mod.rs
   │   └── yaml.rs
   └── error.rs
   ```

### Phase 2: Lexer Implementation (Week 2-3)

**Goal**: Port lexer with Rust-idiomatic design

**Dependencies**:
```toml
nom = "7.1"  # Optional: for parser combinators
```

**Tasks**:
1. Implement lexer state machine
2. Replace Go channels with Rust iterators:
   ```rust
   pub struct Lexer<'a> {
       input: &'a str,
       position: usize,
       current_char: Option<char>,
   }
   
   impl<'a> Iterator for Lexer<'a> {
       type Item = Result<Token, LexError>;
       // ...
   }
   ```
3. Add comprehensive lexer tests
4. Benchmark against Go implementation

### Phase 3: Parser and AST (Week 3-4)

**Goal**: Build parser and AST structures

**Tasks**:
1. Define AST node types using enums:
   ```rust
   pub enum Node {
       And(Box<Node>, Box<Node>),
       Or(Box<Node>, Box<Node>),
       Not(Box<Node>),
       Leaf(Pattern),
   }
   ```
2. Implement parser with error recovery
3. Optimize AST construction
4. Add parser property tests

### Phase 4: Pattern Matching Engine (Week 4-5)

**Goal**: Implement high-performance pattern matching

**Dependencies**:
```toml
aho-corasick = "1.1"  # For multi-pattern string matching
memchr = "2.7"       # For fast byte searching
```

**Tasks**:
1. Port string matchers with optimizations:
   ```rust
   pub enum StringMatcher {
       Exact(String),
       Prefix(String),
       Suffix(String),
       Contains(String),
       Regex(regex::Regex),
       Glob(glob::Pattern),
   }
   ```
2. Implement numeric matchers
3. Add SIMD optimizations where applicable
4. Benchmark pattern matching performance

### Phase 5: Rule Management (Week 5-6)

**Goal**: YAML parsing and rule organization

**Tasks**:
1. Implement rule structures:
   ```rust
   #[derive(Serialize, Deserialize)]
   pub struct Rule {
       pub title: String,
       pub description: String,
       pub logsource: LogSource,
       pub detection: Detection,
       pub tags: Vec<String>,
   }
   ```
2. Add rule validation logic
3. Implement ruleset management with concurrent loading
4. Add rule caching for performance

### Phase 6: Event Processing (Week 6-7)

**Goal**: Efficient event matching system

**Dependencies**:
```toml
rayon = "1.8"  # For parallel processing
```

**Tasks**:
1. Implement event types with zero-copy where possible
2. Add batch processing capabilities:
   ```rust
   pub struct RuleEngine {
       ruleset: Arc<RuleSet>,
   }
   
   impl RuleEngine {
       pub fn process_batch(&self, events: &[Event]) -> Vec<Results> {
           events.par_iter()
               .map(|event| self.evaluate(event))
               .collect()
       }
   }
   ```
3. Optimize memory allocation patterns
4. Add event streaming support

### Phase 7: Redpanda Integration (Week 7-8)

**Goal**: Native Redpanda integration

**Dependencies**:
```toml
rdkafka = { version = "0.36", features = ["tokio"] }
tokio = { version = "1.35", features = ["full"] }
futures = "0.3"
```

**Tasks**:
1. Implement Kafka consumer/producer:
   ```rust
   pub struct RedpandaEventSource {
       consumer: StreamConsumer,
       topic: String,
   }
   
   impl Stream for RedpandaEventSource {
       type Item = Result<Event, Error>;
       // ...
   }
   ```
2. Add backpressure handling
3. Implement checkpoint management
4. Add metrics collection

### Phase 8: Performance Optimization (Week 8-9)

**Goal**: Optimize for production workloads

**Dependencies**:
```toml
criterion = { version = "0.5", features = ["html_reports"] }
flamegraph = "0.6"
```

**Tasks**:
1. Profile and optimize hot paths
2. Implement rule precompilation:
   ```rust
   pub struct CompiledRule {
       matcher: Box<dyn Matcher>,
       metadata: RuleMetadata,
   }
   ```
3. Add JIT compilation for complex rules (optional)
4. Optimize memory usage patterns
5. Add comprehensive benchmarks

### Phase 9: Testing and Documentation (Week 9-10)

**Goal**: Ensure reliability and usability

**Tasks**:
1. Port all Go tests to Rust
2. Add property-based tests:
   ```rust
   #[cfg(test)]
   mod tests {
       use proptest::prelude::*;
       
       proptest! {
           #[test]
           fn test_parser_roundtrip(input: String) {
               // Test parser invariants
           }
       }
   }
   ```
3. Add fuzzing tests
4. Write comprehensive documentation
5. Create migration guide from Go version

## 4. Memory Safety Improvements

1. **Zero-Copy Parsing**: Use `&str` references where possible
2. **Arena Allocation**: For AST nodes to reduce fragmentation
3. **Lifetime Management**: Explicit lifetimes for borrowed data
4. **Safe Concurrency**: Use `Arc<T>` and `Mutex<T>` appropriately

## 5. Error Handling Strategy

1. **Type-Safe Errors**:
   ```rust
   #[derive(Error, Debug)]
   pub enum SigmaError {
       #[error("Parse error: {0}")]
       Parse(String),
       
       #[error("Rule not found: {0}")]
       RuleNotFound(String),
       
       #[error("Invalid pattern: {0}")]
       InvalidPattern(#[from] regex::Error),
   }
   ```

2. **Result-based API**: All fallible operations return `Result<T, E>`
3. **Error Context**: Use `anyhow` for adding context
4. **Graceful Degradation**: Continue processing on non-critical errors

## 6. Testing Strategy

1. **Unit Tests**: For each module
2. **Integration Tests**: End-to-end rule processing
3. **Performance Tests**: Using `criterion`
4. **Fuzz Testing**: For parser robustness
5. **Property Tests**: For algorithmic correctness
6. **Compatibility Tests**: Compare results with Go version

## 7. Tokio Stack Integration

### Full Stack Usage

1. **Tokio Runtime**: Core async runtime for all operations
   ```rust
   #[tokio::main]
   async fn main() -> Result<()> {
       let runtime = tokio::runtime::Builder::new_multi_thread()
           .worker_threads(num_cpus::get())
           .enable_all()
           .build()?;
   }
   ```

2. **Hyper**: HTTP server for health checks and metrics
   ```rust
   use hyper::{Body, Request, Response, Server};
   use tower::ServiceBuilder;
   
   async fn health_server() -> Result<()> {
       let service = ServiceBuilder::new()
           .timeout(Duration::from_secs(30))
           .service(health_service);
       
       Server::bind(&"0.0.0.0:8080".parse()?)
           .serve(service)
           .await?;
   }
   ```

3. **Tonic**: gRPC service for control plane
   ```rust
   use tonic::{transport::Server, Request, Response, Status};
   
   #[tonic::async_trait]
   impl sigma_service_server::SigmaService for SigmaServer {
       async fn process_event(
           &self,
           request: Request<Event>,
       ) -> Result<Response<MatchResult>, Status> {
           // Process event with Sigma rules
       }
   }
   ```

4. **Tower**: Middleware for all services
   ```rust
   use tower::{
       ServiceBuilder,
       timeout::TimeoutLayer,
       retry::RetryLayer,
       limit::RateLimitLayer,
   };
   
   let middleware = ServiceBuilder::new()
       .layer(TimeoutLayer::new(Duration::from_secs(10)))
       .layer(RetryLayer::new(retry_policy))
       .layer(RateLimitLayer::new(100, Duration::from_secs(1)));
   ```

5. **Tracing**: Structured logging throughout
   ```rust
   use tracing::{info, instrument, span, Level};
   
   #[instrument(skip(event))]
   async fn process_event(event: &Event) -> Result<MatchResult> {
       let span = span!(Level::INFO, "rule_matching");
       let _enter = span.enter();
       
       info!(event_id = %event.id, "Processing event");
       // ... processing logic
   }
   ```

6. **Bytes**: Zero-copy event handling
   ```rust
   use bytes::{Bytes, BytesMut};
   
   #[derive(Clone)]
   struct EventData {
       raw: Bytes,
       parsed: Option<Arc<ParsedEvent>>,
   }
   
   impl EventData {
       fn parse(&mut self) -> Result<&ParsedEvent> {
           if self.parsed.is_none() {
               self.parsed = Some(Arc::new(parse_event(&self.raw)?));
           }
           Ok(&self.parsed.as_ref().unwrap())
       }
   }
   ```

### Architecture with Tokio Stack

```rust
// Main service structure
pub struct SigmaService {
    engine: Arc<RuleEngine>,
    kafka_consumer: StreamConsumer,
    health_server: JoinHandle<()>,
    metrics_server: JoinHandle<()>,
    grpc_server: JoinHandle<()>,
}

impl SigmaService {
    pub async fn new(config: Config) -> Result<Self> {
        // Initialize tracing
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .json()
            .init();
        
        // Setup components with Tower middleware
        let engine = Arc::new(RuleEngine::new(config.rules)?);
        
        // Start health/metrics server with Hyper
        let health_server = tokio::spawn(health_server());
        
        // Start gRPC control plane with Tonic
        let grpc_server = tokio::spawn(grpc_control_plane(engine.clone()));
        
        Ok(Self {
            engine,
            kafka_consumer,
            health_server,
            metrics_server,
            grpc_server,
        })
    }
    
    pub async fn run(self) -> Result<()> {
        // Main event processing loop
        let mut event_stream = self.kafka_consumer
            .stream()
            .map_err(|e| anyhow::anyhow!("Kafka error: {}", e));
        
        while let Some(message) = event_stream.try_next().await? {
            let event = Event::from_bytes(message.payload())?;
            
            tokio::spawn(async move {
                match self.engine.process(event).await {
                    Ok(result) => {
                        tracing::info!("Processed event: {:?}", result);
                    }
                    Err(e) => {
                        tracing::error!("Failed to process event: {}", e);
                    }
                }
            });
        }
        
        Ok(())
    }
}
```

## 8. Deployment Considerations

1. **Binary Size**: Optimize with `lto = true`
2. **WASM Support**: Consider `wasm32-unknown-unknown` target
3. **C API**: Expose C-compatible API for interop
4. **Configuration**: Support YAML/TOML configuration
5. **Container**: Multi-stage Docker build with distroless base

## 9. Performance Targets

1. **Throughput**: 2x improvement over Go version
2. **Latency**: < 100μs p99 for rule evaluation
3. **Memory**: 50% reduction in memory usage
4. **Startup**: < 1s for 10,000 rules

## 10. Migration Path

1. **Compatibility Layer**: Implement Go-compatible API
2. **Gradual Migration**: Support running both engines
3. **Data Format**: Maintain YAML rule compatibility
4. **Tooling**: Provide migration scripts

## 11. Milestone Schedule

- **Month 1**: Core implementation (Phases 1-3)
- **Month 2**: Feature parity (Phases 4-6)
- **Month 3**: Integration and optimization (Phases 7-8)
- **Month 4**: Testing and documentation (Phase 9)

## Dependencies Summary

```toml
[dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
regex = "1.10"
glob = "0.3"
thiserror = "1.0"
anyhow = "1.0"

# Tokio Stack
tokio = { version = "1.35", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec", "io"] }
hyper = { version = "1.1", features = ["server", "http2"] }
tonic = { version = "0.11", features = ["transport", "tls"] }
tower = { version = "0.4", features = ["timeout", "retry", "util"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
bytes = "1.5"

# Performance
aho-corasick = "1.1"
memchr = "2.7"

# Async/Streaming
futures = "0.3"
rdkafka = { version = "0.36", features = ["tokio"] }
async-trait = "0.1"

# Optional
nom = "7.1"
pin-project = "1.1"

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1.4"
tokio-test = "0.4"
```

## Success Metrics

1. **Functional**: 100% rule compatibility
2. **Performance**: 2x throughput improvement
3. **Reliability**: 99.99% uptime in production
4. **Maintainability**: Comprehensive test coverage
5. **Usability**: Clear documentation and examples
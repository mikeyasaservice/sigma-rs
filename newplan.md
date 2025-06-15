# Sigma-rs Architecture Redesign Plan

## Executive Summary

Based on analysis of the actual SigmaHQ ruleset (3,500+ rules), we need to fundamentally redesign our approach. The rules are 93.5% complex with heavy string pattern matching (2,460 contains, 1,352 endswith operations). Our current DataFusion-based approach won't achieve the required 3M events/sec performance target.

## Current State Analysis

### Rule Complexity Profile (from SigmaHQ analysis)
- **Total rules**: ~3,500
- **Simple rules**: 48 (1.4%)
- **Complex rules**: 3,452 (93.5%)
- **String operations**:
  - `contains`: 2,460 uses
  - `endswith`: 1,352 uses
  - `startswith`: 137 uses
  - `all` (match all items): 624 uses
  - `re` (regex): 49 uses

### Current Architecture Problems
1. **DataFusion overhead**: Built for complex analytics, not pattern matching
2. **Sequential rule evaluation**: Each rule evaluated independently
3. **No string operation optimization**: Using generic SQL LIKE operations
4. **Temporary table overhead**: Register/deregister for each batch

### Performance Reality Check
- **Current**: ~50-100K events/sec (estimated)
- **Target**: 3M events/sec
- **Gap**: 30-60x performance improvement needed

## New Architecture Design

### Core Principle: Right Tool for Right Job

```
┌─────────────────┐
│   Event Stream  │
└────────┬────────┘
         │
    ┌────▼────┐
    │  Router  │ (By log source: Windows/Linux/Network)
    └────┬────┘
         │
┌────────▼────────┐
│  Sigma Engine   │ Purpose: High-speed pattern matching
│  (Optimized)    │ Output: Enriched events with matches
└────────┬────────┘
         │
┌────────▼────────┐
│     Arroyo      │ Purpose: Behavioral analytics
│                 │ Output: Complex alerts
└─────────────────┘
```

## Phase 1: Sigma Engine Optimization (Target: 1M events/sec)

### 1.1 Rule Preprocessing & Compilation

```rust
// New rule compiler that extracts and groups patterns
pub struct OptimizedRuleCompiler {
    // Group patterns by field and type
    patterns_by_field: HashMap<String, FieldPatterns>,
}

pub struct FieldPatterns {
    // Aho-Corasick for substring matching (handles 2,460 contains)
    substring_matcher: AhoCorasick,
    // Optimized suffix tree for endswith (handles 1,352 endswith)
    suffix_matcher: SuffixTree,
    // Trie for startswith (handles 137 startswith)
    prefix_matcher: Trie,
    // Compiled regex patterns (handles 49 regex)
    regex_matchers: Vec<Regex>,
}
```

### 1.2 String Matching Optimization

**Key Insight**: Group all patterns by field and match type, then evaluate in one pass.

```rust
// Instead of evaluating each rule separately:
// Rule 1: CommandLine contains "powershell"
// Rule 2: CommandLine contains "encoded"
// Rule 3: CommandLine contains "bypass"

// Build one Aho-Corasick matcher for CommandLine:
let cmd_matcher = AhoCorasick::new(&["powershell", "encoded", "bypass"]);
// One pass finds all matches
let matches = cmd_matcher.find_iter(event.command_line);
```

### 1.3 Three-Tier Rule Evaluation

```rust
pub enum CompiledRule {
    // Tier 1: Simple field equality (< 5% of rules)
    Simple {
        field: String,
        value: Value,
    },
    
    // Tier 2: String pattern matching (90% of rules)
    Pattern {
        required_fields: Vec<String>,
        pattern_refs: Vec<PatternRef>, // References to pre-built matchers
        logic: BooleanExpression,
    },
    
    // Tier 3: Complex expressions (< 5% of rules)
    Complex {
        datafusion_expr: Expr, // Fallback to DataFusion
    }
}
```

### 1.4 Batch Processing Strategy

```rust
// Larger batches to amortize overhead
const OPTIMAL_BATCH_SIZE: usize = 256 * 1024; // 256K events

// Process in stages
impl BatchProcessor {
    async fn process_batch(&self, events: RecordBatch) -> MatchResults {
        // Stage 1: Extract frequently accessed fields into columnar format
        let field_columns = self.extract_hot_fields(&events);
        
        // Stage 2: Run pattern matchers on columns
        let pattern_matches = self.run_pattern_matchers(&field_columns);
        
        // Stage 3: Evaluate rule logic using pattern results
        let rule_matches = self.evaluate_rules(&pattern_matches);
        
        rule_matches
    }
}
```

## Phase 2: Distributed Architecture (Target: 3M events/sec)

### 2.1 Rule Partitioning Strategy

```yaml
# Partition rules by log source
Partition 1 (Windows Process): ~1,200 rules
  - EventID: 1, 4688
  - Fields: CommandLine, Image, ParentImage

Partition 2 (Windows Security): ~800 rules
  - EventID: 4624, 4625, 4672
  - Fields: LogonType, TargetUserName

Partition 3 (Network): ~600 rules
  - Fields: DestinationIP, DestinationPort, Protocol

Partition 4 (Linux): ~500 rules
  - Fields: exe, comm, syscall
```

### 2.2 Worker Architecture

```rust
// Each worker handles specific rule partitions
pub struct PartitionedWorker {
    partition_id: u32,
    rules: Vec<CompiledRule>,
    string_matchers: PartitionMatchers,
    
    // Pre-built for this partition's fields
    field_extractors: FieldExtractors,
}

// Router sends events to appropriate workers
pub struct EventRouter {
    // Quick routing based on event type
    routing_table: HashMap<EventType, Vec<WorkerId>>,
}
```

### 2.3 Zero-Copy Event Distribution

```rust
// Use Arrow Flight for zero-copy event distribution
impl EventDistributor {
    async fn distribute(&self, batch: RecordBatch) {
        // Slice batch by event type (zero-copy)
        let partitioned = self.partition_by_type(&batch);
        
        // Send slices to workers via Arrow Flight
        for (worker_id, slice) in partitioned {
            self.flight_client.send_batch(worker_id, slice).await?;
        }
    }
}
```

## Phase 3: Arroyo Integration

### 3.1 Clean Interface

```rust
// Sigma engine outputs enriched events
pub struct EnrichedEvent {
    original_event: RecordBatch,
    matched_rules: Vec<RuleId>,
    risk_score: f32,
    extracted_iocs: Vec<IOC>,
}

// Arroyo consumes for behavioral analysis
CREATE TABLE enriched_events (
    timestamp TIMESTAMP,
    hostname STRING,
    user STRING,
    process STRING,
    matched_rules ARRAY<STRING>,
    risk_score FLOAT
) WITH (
    connector = 'kafka',
    topic = 'sigma-enriched-events'
);

-- Behavioral detection example
SELECT 
    hostname,
    user,
    COUNT(DISTINCT process) as unique_processes,
    SUM(risk_score) as total_risk
FROM enriched_events
GROUP BY hostname, user, HOP(INTERVAL '5' MINUTE)
HAVING unique_processes > 20 OR total_risk > 100;
```

### 3.2 Keep Concerns Separated

- **Sigma Engine**: Single event pattern matching, IOC extraction
- **Arroyo**: Temporal analytics, behavioral patterns, aggregations

## Implementation Timeline

### Week 1-2: Prototype String Matching Optimization
- [ ] Implement Aho-Corasick field grouping
- [ ] Benchmark against current implementation
- [ ] Validate with top 100 most common rules

### Week 3-4: Build Tiered Rule Evaluation
- [ ] Implement rule analyzer and categorizer
- [ ] Build Tier 2 pattern matching engine
- [ ] Create benchmarks for each tier

### Week 5-6: Optimize Batch Processing
- [ ] Increase batch size to 256K
- [ ] Implement columnar field extraction
- [ ] Add zero-copy optimizations

### Week 7-8: Distributed Architecture
- [ ] Implement rule partitioning
- [ ] Build event router
- [ ] Add Arrow Flight distribution

### Week 9-10: Arroyo Integration
- [ ] Define enriched event schema
- [ ] Build Kafka bridge
- [ ] Create example behavioral rules

### Week 11-12: Production Hardening
- [ ] Load testing at scale
- [ ] Monitoring and metrics
- [ ] Documentation

## Success Metrics

### Performance Targets
- **Phase 1**: 1M events/sec single node
- **Phase 2**: 3M events/sec distributed (8 nodes)
- **End-to-end latency**: < 100ms (event → alert)

### Accuracy Targets
- **False positive rate**: < 0.1%
- **Rule coverage**: 100% of SigmaHQ rules supported

### Operational Targets
- **Memory usage**: < 32GB per node
- **CPU efficiency**: > 80% utilization
- **Network overhead**: < 10% of processing time

## Risk Mitigation

### Risk 1: String Matching Complexity
- **Mitigation**: Use proven libraries (Aho-Corasick, Hyperscan)
- **Fallback**: Reduce rule set to most critical rules

### Risk 2: Distributed Coordination Overhead
- **Mitigation**: Stateless workers, Kafka coordination
- **Fallback**: Larger but fewer nodes

### Risk 3: Arroyo Integration Complexity
- **Mitigation**: Simple schema, clear boundaries
- **Fallback**: Direct Kafka → Arroyo without enrichment

## Conclusion

This plan addresses the core issue: SigmaHQ rules are predominantly complex string matching problems, not analytical queries. By building a purpose-built string matching engine and reserving DataFusion/Arroyo for actual analytics, we can achieve our performance targets while maintaining full rule compatibility.

The key insight is that **we're not building a general-purpose rule engine, we're building a Sigma rule engine**, and Sigma rules have specific patterns we can optimize for.
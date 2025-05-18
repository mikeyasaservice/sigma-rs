# Rule Implementation Plan

## Phase 1: Core Rule Structure (Current Task)

### 1.1 Create Basic Rule Types
- [ ] Implement `rule/mod.rs` with Rule struct
- [ ] Implement `rule/detection.rs` with Detection type
- [ ] Implement `rule/logsource.rs` with Logsource type
- [ ] Implement `rule/tags.rs` with helper functions
- [ ] Add YAML deserialization support

### 1.2 Create RuleHandle
- [ ] Add RuleHandle struct with file metadata
- [ ] Implement multipart detection
- [ ] Add path tracking for debugging

### 1.3 Add Result Types
- [ ] Implement `result/mod.rs` with Result struct
- [ ] Add Results collection type
- [ ] Include tag propagation

## Phase 2: Tree Integration

### 2.1 Tree Structure
- [ ] Create `tree/mod.rs` with Tree struct
- [ ] Link to existing AST Branch trait
- [ ] Add evaluation methods

### 2.2 Tree Builder
- [ ] Port `newBranch` logic from Go
- [ ] Integrate with existing parser
- [ ] Handle detection field extraction

### 2.3 Pattern Matching
- [ ] Implement wildcard pattern support
- [ ] Add "all of" and "1 of" logic
- [ ] Support nested conditions

## Phase 3: Ruleset Implementation

### 3.1 Core Ruleset
- [ ] Create `ruleset/mod.rs` with Ruleset struct
- [ ] Add thread-safe rule storage
- [ ] Implement statistics tracking

### 3.2 Configuration
- [ ] Create `ruleset/config.rs`
- [ ] Add directory validation
- [ ] Support error handling modes

### 3.3 File Loading
- [ ] Create `ruleset/loader.rs`
- [ ] Implement recursive YAML scanning
- [ ] Add bulk loading with error collection

### 3.4 Evaluation
- [ ] Implement `EvalAll` method
- [ ] Add parallel evaluation support
- [ ] Collect results from matches

## Phase 4: Error Handling

### 4.1 Port Error Types
- [ ] Add rule-specific errors
- [ ] Implement bulk error handling
- [ ] Add context to parse errors

### 4.2 Resilient Loading
- [ ] Skip invalid rules optionally
- [ ] Collect and report errors
- [ ] Maintain statistics

## Phase 5: Testing

### 5.1 Unit Tests
- [ ] Test YAML parsing
- [ ] Test rule matching
- [ ] Test error scenarios

### 5.2 Integration Tests
- [ ] Test full rule loading
- [ ] Test evaluation pipeline
- [ ] Compare with Go implementation

### 5.3 Benchmarks
- [ ] Rule loading performance
- [ ] Evaluation throughput
- [ ] Memory usage comparison

## Implementation Order

1. Start with basic Rule structure and YAML parsing
2. Add Detection and Logsource types
3. Implement RuleHandle with metadata
4. Create Result types
5. Integrate with existing Tree/AST
6. Build Ruleset with loading
7. Add evaluation methods
8. Comprehensive testing

## Key Decisions

1. Use `serde_yaml` for YAML parsing (matches Go's yaml.v2)
2. Use `Arc` for shared ownership of rules
3. Use `RwLock` for thread-safe ruleset access
4. Keep AST nodes separate from rule metadata
5. Support graceful error handling by default
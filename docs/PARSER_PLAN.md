# Parser Implementation Plan

## Phase 1: Core Structure (Day 1)

### 1.1 Create Module Structure
```bash
mkdir -p src/parser
mkdir -p src/detection
mkdir -p src/ast
```

### 1.2 Define Core Types
- Parser struct
- Detection type (HashMap<String, Value>)
- Error types
- Basic AST traits

## Phase 2: Token Validation (Day 1-2)

### 2.1 Port Token Sequence Validation
- Implement `valid_token_sequence` function
- Create validation state machine
- Add comprehensive error reporting

### 2.2 Implement Collection Phase
- Async token collection from lexer
- Sequence validation during collection
- Error accumulation

## Phase 3: AST Construction (Day 2-3)

### 3.1 Define AST Node Types
- Branch trait
- Node enum variants (And, Or, Not, SimpleAnd, SimpleOr)
- Field matcher implementation

### 3.2 Implement Branch Builder
- Port `new_branch` recursive function
- Group extraction logic
- Operator precedence handling

## Phase 4: Wildcard Support (Day 3-4)

### 4.1 Glob Pattern Matching
- Integration with glob crate
- Wildcard identifier extraction
- "all of" and "1 of" pattern handling

### 4.2 Dynamic Branch Construction
- Build branches from wildcard matches
- Support negation with wildcards
- Test complex wildcard scenarios

## Phase 5: Integration & Testing (Day 4-5)

### 5.1 Parser-Lexer Integration
- Connect async channels
- End-to-end parsing pipeline
- Error propagation

### 5.2 Comprehensive Testing
- Port all Go test cases
- Add Rust-specific tests
- Property-based testing
- Benchmark implementation

## Phase 6: Optimization (Day 5)

### 6.1 Performance Tuning
- Profile hot paths
- Optimize allocations
- Improve async performance

### 6.2 Memory Efficiency
- Implement zero-copy where possible
- Use arena allocation for AST
- Minimize cloning

## Implementation Order

1. **parser/error.rs** - Error types
2. **detection/mod.rs** - Detection type
3. **parser/validate.rs** - Token validation
4. **ast/mod.rs** - AST traits and nodes
5. **parser/mod.rs** - Core parser
6. **parser/wildcard.rs** - Wildcard support
7. **tests/parser_tests.rs** - Integration tests

## Milestones

- [ ] Core parser structure complete
- [ ] Token validation working
- [ ] Basic AST construction
- [ ] Wildcard patterns supported
- [ ] All Go tests passing
- [ ] Performance benchmarks met

## Risk Mitigation

1. **Complex Grammar**: Start with simple cases, gradually add complexity
2. **Async Complexity**: Use tokio best practices, avoid deadlocks
3. **Memory Usage**: Profile early and often
4. **Compatibility**: Continuously test against Go implementation

## Code Examples

### Parser Initialization
```rust
let mut parser = Parser::new(lexer, condition);
parser.run().await?;
let ast = parser.result()?;
```

### AST Matching
```rust
let event = Event::from_json(data)?;
let (matched, applicable) = ast.matches(&event);
```

### Error Handling
```rust
match parser.run().await {
    Ok(()) => process_ast(parser.result()?),
    Err(ParseError::InvalidSequence { .. }) => handle_syntax_error(),
    Err(e) => return Err(e.into()),
}
```
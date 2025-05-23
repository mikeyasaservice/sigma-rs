# Sigma Parser Architecture (Rust)

This document outlines the architecture for the Rust implementation of the Sigma parser, which consumes tokens from the lexer and builds an Abstract Syntax Tree (AST).

## Overview

The parser follows a two-phase approach:
1. **Collection Phase**: Gather tokens from lexer, validate sequences
2. **Parse Phase**: Build AST recursively from validated tokens

## Core Components

### 1. Parser Structure

```rust
pub struct Parser {
    lexer: Lexer,
    tokens: Vec<Item>,
    previous: Option<Item>,
    sigma: Detection,
    condition: String,
    result: Option<Box<dyn Branch>>,
    no_collapse_ws: bool,
}
```

### 2. AST Node Types

```rust
// Base trait for all AST nodes
pub trait Branch: Send + Sync {
    fn matches(&self, event: &Event) -> MatchResult;
}

// Node types matching Go implementation
pub enum AstNode {
    And(Box<dyn Branch>, Box<dyn Branch>),
    Or(Box<dyn Branch>, Box<dyn Branch>),
    Not(Box<dyn Branch>),
    SimpleAnd(Vec<Box<dyn Branch>>),
    SimpleOr(Vec<Box<dyn Branch>>),
    FieldMatcher(FieldRule),
}
```

### 3. Detection Type

```rust
use std::collections::HashMap;
use serde_json::Value;

pub type Detection = HashMap<String, Value>;
```

### 4. Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Unsupported token: {msg}")]
    UnsupportedToken { msg: String },
    
    #[error("Invalid token sequence: {prev:?} -> {next:?}")]
    InvalidTokenSequence { prev: Item, next: Item },
    
    #[error("Missing condition item: {key}")]
    MissingConditionItem { key: String },
    
    #[error("Incomplete token sequence")]
    IncompleteTokenSequence { expression: String, last: Item },
}
```

## Key Algorithms

### Token Sequence Validation

The parser validates that tokens appear in valid sequences:
- Identifiers must be preceded by operators or grouping symbols
- Operators require operands
- Parentheses must be balanced

### AST Construction

The `new_branch` function recursively builds the AST:
1. Process tokens sequentially
2. Build AND collections
3. Fold OR operations
4. Handle NOT negations
5. Recurse for grouped expressions
6. Support wildcard patterns

### Wildcard Handling

"1 of" and "all of" patterns:
- Extract matching identifiers using glob patterns
- Build OR branches for "1 of"
- Build AND branches for "all of"

## Async Design

```rust
impl Parser {
    pub async fn run(&mut self) -> Result<(), ParseError> {
        // Phase 1: Collect tokens
        self.collect().await?;
        
        // Phase 2: Build AST
        self.parse()?;
        
        Ok(())
    }
    
    async fn collect(&mut self) -> Result<(), ParseError> {
        let mut rx = self.lexer.scan().await?;
        
        while let Some(item) = rx.recv().await {
            self.validate_sequence(&item)?;
            self.tokens.push(item);
            self.previous = Some(item);
        }
        
        Ok(())
    }
}
```

## Performance Considerations

1. **Arena Allocation**: Use arena allocator for AST nodes
2. **Zero-Copy Strings**: Use `Cow<str>` for string values
3. **Parallel Evaluation**: Concurrent branch matching
4. **Token Buffering**: Efficient channel communication

## Testing Strategy

1. Port all Go test cases
2. Add Rust-specific edge cases
3. Benchmark against Go implementation
4. Property-based testing for AST construction

## Module Organization

```
src/
├── parser/
│   ├── mod.rs         # Parser implementation
│   ├── ast.rs         # AST node types
│   ├── error.rs       # Error definitions
│   ├── validate.rs    # Token sequence validation
│   └── wildcard.rs    # Wildcard pattern handling
└── detection/
    ├── mod.rs         # Detection type and utilities
    └── field.rs       # Field matching logic
```

## Next Steps

1. Implement core parser structure
2. Port token validation logic
3. Implement AST construction
4. Add wildcard support
5. Create comprehensive tests
6. Benchmark and optimize
# Rust Sigma Parser Architecture

## Overview

This document outlines the architecture for implementing a Sigma rule parser in Rust, maintaining compatibility with the existing Go implementation while leveraging Rust's type system and async capabilities.

## Core Components

### 1. Token and Lexer Layer

```rust
// Token definitions matching Go implementation
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Identifiers
    Identifier(String),
    IdentifierWithWildcard(String),
    IdentifierAll,
    
    // Keywords
    KeywordAnd,
    KeywordOr,
    KeywordNot,
    
    // Statements
    StmtAll,  // "all of"
    StmtOne,  // "1 of"
    
    // Separators
    SepLpar,  // (
    SepRpar,  // )
    SepPipe,  // |
    
    // Operators
    OpEq,     // =
    OpGt,     // >
    OpGte,    // >=
    OpLt,     // <
    OpLte,    // <=
    
    // Special
    LitEof,
    Unsupported(String),
    Error(String),
}

// Lexer state machine
pub struct Lexer {
    input: Vec<char>,
    position: usize,
    read_position: usize,
    current_char: char,
}

impl Lexer {
    pub fn new(input: &str) -> Self;
    pub async fn next_token(&mut self) -> Result<Token, LexerError>;
}
```

### 2. Parser Structure

```rust
use std::collections::HashMap;
use serde_json::Value;

pub struct Parser {
    lexer: Lexer,
    tokens: Vec<(Token, String)>,
    previous: Token,
    detection: Detection,
    condition: String,
    result: Option<Box<dyn Branch>>,
    no_collapse_ws: bool,
}

pub type Detection = HashMap<String, Value>;

impl Parser {
    pub async fn new(condition: &str, detection: Detection, no_collapse_ws: bool) -> Self;
    pub async fn parse(&mut self) -> Result<Box<dyn Branch>, ParserError>;
    
    // Internal methods
    async fn collect_tokens(&mut self) -> Result<(), ParserError>;
    async fn build_ast(&mut self) -> Result<Box<dyn Branch>, ParserError>;
}
```

### 3. AST Node Types

```rust
#[async_trait]
pub trait Branch: Send + Sync {
    async fn match_event(&self, event: &Event) -> (bool, bool);
}

// Simple nodes for logical operations
pub struct NodeAnd {
    left: Box<dyn Branch>,
    right: Box<dyn Branch>,
}

pub struct NodeOr {
    left: Box<dyn Branch>,
    right: Box<dyn Branch>,
}

pub struct NodeNot {
    branch: Box<dyn Branch>,
}

// Collection nodes for multiple branches
pub struct NodeSimpleAnd(Vec<Box<dyn Branch>>);
pub struct NodeSimpleOr(Vec<Box<dyn Branch>>);

// Implement async_trait for all nodes
#[async_trait]
impl Branch for NodeAnd {
    async fn match_event(&self, event: &Event) -> (bool, bool) {
        let (l_match, l_applicable) = self.left.match_event(event).await;
        if !l_match {
            return (false, l_applicable);
        }
        let (r_match, r_applicable) = self.right.match_event(event).await;
        (l_match && r_match, l_applicable && r_applicable)
    }
}
```

### 4. Pattern Matching System

```rust
pub trait Matcher: Send + Sync {
    fn matches(&self, value: &str) -> bool;
}

#[derive(Debug)]
pub enum TextPatternModifier {
    None,
    Contains,
    Prefix,
    Suffix,
    All,
    Regex,
    Keyword,
}

pub struct StringMatcher {
    patterns: Vec<Box<dyn Matcher>>,
    modifier: TextPatternModifier,
    lowercase: bool,
    no_collapse_ws: bool,
}

// Concrete pattern implementations
pub struct ContentPattern {
    token: String,
    lowercase: bool,
    no_collapse_ws: bool,
}

pub struct GlobPattern {
    glob: glob::Pattern,
    no_collapse_ws: bool,
}

pub struct RegexPattern {
    regex: regex::Regex,
}
```

### 5. Rule and Event Processing

```rust
pub struct Rule {
    pub id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub level: String,
    pub tags: Vec<String>,
    pub logsource: LogSource,
    pub detection: Detection,
}

pub struct LogSource {
    pub product: Option<String>,
    pub category: Option<String>,
    pub service: Option<String>,
}

#[derive(Deserialize)]
pub struct Event {
    data: HashMap<String, Value>,
}

impl Event {
    pub fn select(&self, key: &str) -> Option<&Value>;
    pub fn keywords(&self) -> Option<Vec<String>>;
}
```

### 6. Core Parsing Logic

```rust
impl Parser {
    async fn parse_expression(&mut self, tokens: &[Token]) -> Result<Box<dyn Branch>, ParserError> {
        let mut and_branches = Vec::new();
        let mut or_branches = Vec::new();
        let mut negated = false;
        let mut wildcard = None;
        
        for token in tokens {
            match token {
                Token::Identifier(name) => {
                    let branch = self.create_identifier_branch(name).await?;
                    and_branches.push(if negated {
                        Box::new(NodeNot { branch })
                    } else {
                        branch
                    });
                    negated = false;
                }
                
                Token::KeywordAnd => { /* Continue collection */ }
                
                Token::KeywordOr => {
                    // Push collected AND branches to OR
                    or_branches.push(self.reduce_and_branches(and_branches)?);
                    and_branches = Vec::new();
                }
                
                Token::KeywordNot => {
                    negated = true;
                }
                
                Token::SepLpar => {
                    // Recursively parse group
                    let group = self.extract_group(tokens)?;
                    let branch = self.parse_expression(&group).await?;
                    and_branches.push(if negated {
                        Box::new(NodeNot { branch })
                    } else {
                        branch
                    });
                    negated = false;
                }
                
                Token::StmtAll => {
                    wildcard = Some(WildcardType::All);
                }
                
                Token::StmtOne => {
                    wildcard = Some(WildcardType::One);
                }
                
                Token::IdentifierWithWildcard(pattern) => {
                    let branches = self.handle_wildcard(pattern, wildcard).await?;
                    and_branches.push(if negated {
                        Box::new(NodeNot { branch: branches })
                    } else {
                        branches
                    });
                    negated = false;
                    wildcard = None;
                }
                
                _ => return Err(ParserError::UnexpectedToken),
            }
        }
        
        // Final reduction
        or_branches.push(self.reduce_and_branches(and_branches)?);
        self.reduce_or_branches(or_branches)
    }
}
```

### 7. Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Lexer error: {0}")]
    LexerError(#[from] LexerError),
    
    #[error("Invalid token sequence: {prev:?} -> {next:?}")]
    InvalidTokenSequence { prev: Token, next: Token },
    
    #[error("Missing condition item: {key}")]
    MissingConditionItem { key: String },
    
    #[error("Unexpected token: {0:?}")]
    UnexpectedToken(Token),
    
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum LexerError {
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
    
    #[error("Unterminated string")]
    UnterminatedString,
}
```

### 8. Async Design Patterns

The parser will use async/await to support:

1. **Streaming token generation**: Tokens are generated asynchronously from the lexer
2. **Concurrent branch evaluation**: Multiple branches can be evaluated concurrently
3. **Non-blocking I/O**: For rule loading and event processing

```rust
pub struct AsyncParser {
    lexer: AsyncLexer,
    token_stream: Pin<Box<dyn Stream<Item = Result<Token, LexerError>> + Send>>,
}

impl AsyncParser {
    pub async fn parse_condition(&mut self) -> Result<Box<dyn Branch>, ParserError> {
        let mut tokens = Vec::new();
        
        // Collect tokens asynchronously
        while let Some(token_result) = self.token_stream.next().await {
            match token_result {
                Ok(token) => tokens.push(token),
                Err(e) => return Err(e.into()),
            }
        }
        
        // Build AST
        self.build_ast_from_tokens(&tokens).await
    }
}
```

### 9. Module Organization

```
src/
├── lib.rs              // Public API
├── lexer.rs            // Lexer implementation
├── parser.rs           // Parser core
├── ast.rs              // AST node definitions
├── patterns.rs         // Pattern matching
├── rules.rs            // Rule and detection structures
├── events.rs           // Event processing
├── errors.rs           // Error types
└── tests/
    ├── lexer_tests.rs
    ├── parser_tests.rs
    └── integration_tests.rs
```

### 10. Testing Strategy

1. **Unit Tests**: For each component (lexer, parser, patterns)
2. **Property-based Tests**: Using `proptest` for parser robustness
3. **Integration Tests**: Full rule parsing and matching
4. **Fuzz Testing**: Using `cargo-fuzz` for edge cases
5. **Compatibility Tests**: Compare output with Go implementation

### 11. Performance Optimizations

1. **String Interning**: Use `string_cache` for common identifiers
2. **Arena Allocation**: Use `bumpalo` for AST node allocation
3. **SIMD Pattern Matching**: Use `simd` features for string matching
4. **Concurrent Processing**: Process multiple rules in parallel

### 12. Key Features to Implement

1. **Wildcard Support**: "all of" and "1 of" patterns
2. **Group Parsing**: Recursive parentheses handling
3. **Whitespace Collapsing**: Optional whitespace normalization
4. **Modifiers**: Support for |contains, |startswith, |endswith, |re
5. **Type Coercion**: Handle numeric and string conversions
6. **Error Recovery**: Continue parsing after errors when possible

### 13. API Design

```rust
// High-level API
pub async fn parse_rule(yaml_content: &str) -> Result<Rule, Error>;
pub async fn compile_condition(condition: &str, detection: Detection) -> Result<Tree, Error>;

// Builder pattern for configuration
pub struct ParserBuilder {
    no_collapse_whitespace: bool,
    error_recovery: bool,
    max_depth: usize,
}

impl ParserBuilder {
    pub fn new() -> Self;
    pub fn no_collapse_whitespace(mut self, value: bool) -> Self;
    pub fn build(self) -> Parser;
}
```

### 14. Go Compatibility Checklist

- [ ] Token types match Go implementation
- [ ] Lexer produces same token sequences
- [ ] Parser validates sequences identically
- [ ] AST structure compatible
- [ ] Pattern matching behavior identical
- [ ] Error messages similar format
- [ ] YAML parsing compatibility
- [ ] Event structure matching

### 15. Future Enhancements

1. **JIT Compilation**: Compile hot paths to native code
2. **Custom Pattern DSL**: Extend pattern matching capabilities
3. **Streaming Parser**: Process large rule sets efficiently
4. **Rule Optimization**: Optimize AST for better performance
5. **WebAssembly Support**: Run parser in browsers

## Implementation Timeline

1. **Phase 1**: Core lexer and token generation (Week 1)
2. **Phase 2**: Basic parser and AST construction (Week 2)
3. **Phase 3**: Pattern matching system (Week 3)
4. **Phase 4**: Full compatibility testing (Week 4)
5. **Phase 5**: Performance optimization (Week 5)
6. **Phase 6**: Documentation and examples (Week 6)
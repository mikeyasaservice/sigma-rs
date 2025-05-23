# Rust Sigma Parser Implementation Plan

## Phase 1: Core Infrastructure (Week 1)

### 1.1 Project Setup
```toml
[dependencies]
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
glob = "0.3"
regex = "1.10"
futures = "0.3"
pin-project = "1.1"
```

### 1.2 Base Types
```rust
// src/types.rs
use std::collections::HashMap;
use serde_json::Value;

pub type Detection = HashMap<String, Value>;
pub type EventData = HashMap<String, Value>;

#[derive(Debug, Clone)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}
```

### 1.3 Error Types
```rust
// src/errors.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SigmaError {
    #[error("Lexer error at {position:?}: {message}")]
    LexerError {
        message: String,
        position: Position,
    },
    
    #[error("Parser error: {0}")]
    ParserError(String),
    
    #[error("Pattern compilation error: {0}")]
    PatternError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

## Phase 2: Lexer Implementation (Week 1-2)

### 2.1 Token Stream Design
```rust
// src/lexer.rs
use futures::stream::{Stream, StreamExt};
use pin_project::pin_project;

#[pin_project]
pub struct TokenStream {
    #[pin]
    inner: futures::stream::BoxStream<'static, Result<Token, SigmaError>>,
}

impl Stream for TokenStream {
    type Item = Result<Token, SigmaError>;
    
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn tokenize(input: String) -> TokenStream {
        let lexer = Lexer::new(input);
        let stream = futures::stream::unfold(lexer, |mut lexer| async move {
            match lexer.next_token().await {
                Ok(Token::LitEof) => None,
                Ok(token) => Some((Ok(token), lexer)),
                Err(e) => Some((Err(e), lexer)),
            }
        });
        
        TokenStream {
            inner: Box::pin(stream),
        }
    }
}
```

### 2.2 State Machine Implementation
```rust
#[derive(Debug, Clone, Copy)]
enum LexerState {
    Initial,
    InIdentifier,
    InKeyword,
    InOperator,
    InWhitespace,
    InComment,
}

impl Lexer {
    async fn next_token(&mut self) -> Result<Token, SigmaError> {
        self.skip_whitespace().await;
        
        match self.current_char() {
            Some('(') => self.consume_single(Token::SepLpar),
            Some(')') => self.consume_single(Token::SepRpar),
            Some('|') => self.consume_single(Token::SepPipe),
            Some(c) if c.is_alphabetic() => self.read_identifier().await,
            Some(c) if c.is_numeric() => self.read_number().await,
            None => Ok(Token::LitEof),
            Some(c) => Err(SigmaError::LexerError {
                message: format!("Unexpected character: {}", c),
                position: self.current_position(),
            }),
        }
    }
    
    async fn read_identifier(&mut self) -> Result<Token, SigmaError> {
        let start = self.position;
        while let Some(c) = self.current_char() {
            if c.is_alphanumeric() || c == '_' || c == '*' || c == '?' {
                self.advance();
            } else {
                break;
            }
        }
        
        let value = self.input[start..self.position].iter().collect::<String>();
        
        match value.as_str() {
            "and" => Ok(Token::KeywordAnd),
            "or" => Ok(Token::KeywordOr),
            "not" => Ok(Token::KeywordNot),
            "all" if self.peek_ahead("of") => {
                self.consume_keyword("of").await;
                Ok(Token::StmtAll)
            },
            "1" if self.peek_ahead("of") => {
                self.consume_keyword("of").await;
                Ok(Token::StmtOne)
            },
            "them" => Ok(Token::IdentifierAll),
            _ if value.contains('*') || value.contains('?') => {
                Ok(Token::IdentifierWithWildcard(value))
            },
            _ => Ok(Token::Identifier(value)),
        }
    }
}
```

## Phase 3: Parser Architecture (Week 2-3)

### 3.1 Parser Stream Processing
```rust
// src/parser.rs
use futures::stream::{StreamExt, TryStreamExt};

pub struct Parser {
    token_stream: TokenStream,
    current_token: Option<Token>,
    detection: Detection,
    no_collapse_ws: bool,
}

impl Parser {
    pub async fn parse(condition: &str, detection: Detection) -> Result<Tree, SigmaError> {
        let token_stream = Lexer::tokenize(condition.to_string());
        let mut parser = Parser {
            token_stream,
            current_token: None,
            detection,
            no_collapse_ws: false,
        };
        
        parser.parse_condition().await
    }
    
    async fn parse_condition(&mut self) -> Result<Tree, SigmaError> {
        // Advance to first token
        self.advance().await?;
        
        let root = self.parse_expression(0).await?;
        
        // Ensure we've consumed all tokens
        if !matches!(self.current_token, Some(Token::LitEof) | None) {
            return Err(SigmaError::ParserError(
                "Unexpected tokens at end of condition".to_string()
            ));
        }
        
        Ok(Tree { root })
    }
}
```

### 3.2 Precedence Climbing Parser
```rust
impl Parser {
    async fn parse_expression(&mut self, min_precedence: usize) -> Result<Box<dyn Branch>, SigmaError> {
        let mut left = self.parse_primary().await?;
        
        while let Some(token) = &self.current_token {
            let (precedence, is_right_associative) = match token {
                Token::KeywordOr => (1, false),
                Token::KeywordAnd => (2, false),
                _ => break,
            };
            
            if precedence < min_precedence {
                break;
            }
            
            let operator = token.clone();
            self.advance().await?;
            
            let next_min_precedence = if is_right_associative {
                precedence
            } else {
                precedence + 1
            };
            
            let right = self.parse_expression(next_min_precedence).await?;
            
            left = match operator {
                Token::KeywordAnd => Box::new(NodeAnd { left, right }),
                Token::KeywordOr => Box::new(NodeOr { left, right }),
                _ => unreachable!(),
            };
        }
        
        Ok(left)
    }
    
    async fn parse_primary(&mut self) -> Result<Box<dyn Branch>, SigmaError> {
        match &self.current_token {
            Some(Token::KeywordNot) => {
                self.advance().await?;
                let inner = self.parse_primary().await?;
                Ok(Box::new(NodeNot { branch: inner }))
            },
            Some(Token::SepLpar) => {
                self.advance().await?;
                let expr = self.parse_expression(0).await?;
                self.expect_token(Token::SepRpar).await?;
                Ok(expr)
            },
            Some(Token::StmtAll) => self.parse_wildcard(WildcardType::All).await,
            Some(Token::StmtOne) => self.parse_wildcard(WildcardType::One).await,
            Some(Token::Identifier(name)) => {
                let name = name.clone();
                self.advance().await?;
                self.create_identifier_branch(&name).await
            },
            _ => Err(SigmaError::ParserError(
                format!("Unexpected token: {:?}", self.current_token)
            )),
        }
    }
}
```

### 3.3 Wildcard Processing
```rust
impl Parser {
    async fn parse_wildcard(&mut self, wildcard_type: WildcardType) -> Result<Box<dyn Branch>, SigmaError> {
        self.advance().await?; // consume "all of" or "1 of"
        
        match &self.current_token {
            Some(Token::IdentifierAll) => {
                self.advance().await?;
                let branches = self.extract_all_branches().await?;
                match wildcard_type {
                    WildcardType::All => Ok(Box::new(NodeSimpleAnd(branches))),
                    WildcardType::One => Ok(Box::new(NodeSimpleOr(branches))),
                }
            },
            Some(Token::IdentifierWithWildcard(pattern)) => {
                let pattern = pattern.clone();
                self.advance().await?;
                let branches = self.extract_wildcard_branches(&pattern).await?;
                match wildcard_type {
                    WildcardType::All => Ok(Box::new(NodeSimpleAnd(branches))),
                    WildcardType::One => Ok(Box::new(NodeSimpleOr(branches))),
                }
            },
            _ => Err(SigmaError::ParserError(
                "Expected identifier after wildcard statement".to_string()
            )),
        }
    }
    
    async fn extract_wildcard_branches(&self, pattern: &str) -> Result<Vec<Box<dyn Branch>>, SigmaError> {
        let glob = glob::Pattern::new(pattern)
            .map_err(|e| SigmaError::PatternError(e.to_string()))?;
        
        let matching_keys: Vec<String> = self.detection
            .keys()
            .filter(|k| glob.matches(k))
            .cloned()
            .collect();
        
        let branches = futures::stream::iter(matching_keys)
            .then(|key| async move {
                let value = self.detection.get(&key).unwrap();
                self.create_branch_from_value(value, key).await
            })
            .try_collect()
            .await?;
        
        Ok(branches)
    }
}
```

## Phase 4: Pattern Matching (Week 3)

### 4.1 Async Pattern Evaluation
```rust
// src/patterns.rs
#[async_trait]
pub trait AsyncMatcher: Send + Sync {
    async fn matches(&self, value: &str) -> bool;
}

pub struct AsyncStringMatcher {
    patterns: Vec<Box<dyn AsyncMatcher>>,
    conjunction: bool,
}

#[async_trait]
impl AsyncMatcher for AsyncStringMatcher {
    async fn matches(&self, value: &str) -> bool {
        if self.conjunction {
            // All patterns must match
            for pattern in &self.patterns {
                if !pattern.matches(value).await {
                    return false;
                }
            }
            true
        } else {
            // Any pattern can match
            for pattern in &self.patterns {
                if pattern.matches(value).await {
                    return true;
                }
            }
            false
        }
    }
}
```

### 4.2 Pattern Implementation
```rust
pub struct GlobPattern {
    pattern: glob::Pattern,
    no_collapse_ws: bool,
}

#[async_trait]
impl AsyncMatcher for GlobPattern {
    async fn matches(&self, value: &str) -> bool {
        let processed = if self.no_collapse_ws {
            value.to_string()
        } else {
            collapse_whitespace(value)
        };
        self.pattern.matches(&processed)
    }
}

pub struct RegexPattern {
    regex: regex::Regex,
}

#[async_trait]
impl AsyncMatcher for RegexPattern {
    async fn matches(&self, value: &str) -> bool {
        // Could potentially offload regex matching to thread pool for complex patterns
        tokio::task::spawn_blocking({
            let regex = self.regex.clone();
            let value = value.to_string();
            move || regex.is_match(&value)
        })
        .await
        .unwrap_or(false)
    }
}
```

## Phase 5: AST Evaluation (Week 3-4)

### 5.1 Concurrent Branch Evaluation
```rust
// src/ast.rs
#[async_trait]
pub trait Branch: Send + Sync {
    async fn evaluate(&self, event: &Event) -> (bool, bool);
}

pub struct NodeSimpleOr(Vec<Box<dyn Branch>>);

#[async_trait]
impl Branch for NodeSimpleOr {
    async fn evaluate(&self, event: &Event) -> (bool, bool) {
        // Evaluate branches concurrently
        let futures: Vec<_> = self.0
            .iter()
            .map(|branch| branch.evaluate(event))
            .collect();
        
        let results = futures::future::join_all(futures).await;
        
        let mut any_match = false;
        let mut any_applicable = false;
        
        for (matches, applicable) in results {
            if matches {
                return (true, true);
            }
            if applicable {
                any_applicable = true;
            }
        }
        
        (any_match, any_applicable)
    }
}
```

### 5.2 Smart Evaluation Ordering
```rust
#[async_trait]
impl Branch for NodeAnd {
    async fn evaluate(&self, event: &Event) -> (bool, bool) {
        // Short-circuit evaluation for AND
        let (left_match, left_applicable) = self.left.evaluate(event).await;
        if !left_match {
            return (false, left_applicable);
        }
        
        let (right_match, right_applicable) = self.right.evaluate(event).await;
        (left_match && right_match, left_applicable && right_applicable)
    }
}
```

## Phase 6: Integration and Testing (Week 4)

### 6.1 Integration Tests
```rust
// tests/integration_tests.rs
#[tokio::test]
async fn test_complex_rule_parsing() {
    let yaml = r#"
title: Process creation rule
detection:
    selection:
        EventID: 1
        CommandLine|contains: 'cmd.exe'
    filter:
        User: 'SYSTEM'
    condition: selection and not filter
"#;
    
    let rule = parse_rule(yaml).await.unwrap();
    let tree = compile_condition(&rule).await.unwrap();
    
    let event = Event::from_json(r#"{
        "EventID": 1,
        "CommandLine": "cmd.exe /c whoami",
        "User": "john"
    }"#).unwrap();
    
    let (matches, applicable) = tree.evaluate(&event).await;
    assert!(matches);
    assert!(applicable);
}
```

### 6.2 Benchmark Tests
```rust
// benches/parser_bench.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");
    
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("parse_complex_condition", size), size, |b, size| {
            let condition = generate_complex_condition(*size);
            b.to_async(&runtime).iter(|| async {
                Parser::parse(&condition, Detection::new()).await.unwrap()
            });
        });
    }
}
```

## Phase 7: Performance Optimizations (Week 5)

### 7.1 Token Caching
```rust
use std::sync::Arc;
use lru::LruCache;
use tokio::sync::Mutex;

pub struct CachedLexer {
    cache: Arc<Mutex<LruCache<String, Vec<Token>>>>,
    inner: Lexer,
}

impl CachedLexer {
    pub async fn tokenize_cached(&self, input: &str) -> Result<Vec<Token>, SigmaError> {
        // Check cache first
        let mut cache = self.cache.lock().await;
        if let Some(tokens) = cache.get(input) {
            return Ok(tokens.clone());
        }
        
        // Tokenize and cache
        drop(cache);
        let tokens = self.inner.tokenize(input.to_string())
            .try_collect::<Vec<_>>()
            .await?;
        
        let mut cache = self.cache.lock().await;
        cache.put(input.to_string(), tokens.clone());
        Ok(tokens)
    }
}
```

### 7.2 SIMD String Matching
```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub fn simd_contains(haystack: &str, needle: &str) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    
    unsafe {
        // Use SIMD instructions for faster string matching
        // Implementation details...
    }
}
```

## Phase 8: Error Recovery (Week 5)

### 8.1 Partial Parsing
```rust
pub struct RecoverableParser {
    inner: Parser,
    errors: Vec<SigmaError>,
}

impl RecoverableParser {
    pub async fn parse_with_recovery(&mut self) -> Result<PartialTree, Vec<SigmaError>> {
        let mut branches = Vec::new();
        let mut current_tokens = Vec::new();
        
        while let Some(token) = self.next_token_safe().await {
            match self.try_parse_branch(&current_tokens).await {
                Ok(branch) => {
                    branches.push(branch);
                    current_tokens.clear();
                },
                Err(e) => {
                    self.errors.push(e);
                    // Skip to next separator or keyword
                    self.recover_to_sync_point().await;
                }
            }
        }
        
        Ok(PartialTree { branches, errors: self.errors.clone() })
    }
}
```

## Phase 9: CLI and API (Week 6)

### 9.1 CLI Implementation
```rust
// src/bin/sigma-parser.rs
use clap::Parser;
use sigma_rs::{parse_rule, compile_condition};

#[derive(Parser)]
struct Cli {
    #[clap(short, long)]
    rule_file: PathBuf,
    
    #[clap(short, long)]
    event_file: Option<PathBuf>,
    
    #[clap(short, long)]
    validate_only: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    let rule_content = tokio::fs::read_to_string(cli.rule_file).await?;
    let rule = parse_rule(&rule_content).await?;
    
    if cli.validate_only {
        tracing::error!("Rule validated successfully");
        return Ok(());
    }
    
    let tree = compile_condition(&rule).await?;
    
    if let Some(event_file) = cli.event_file {
        let event_content = tokio::fs::read_to_string(event_file).await?;
        let event = Event::from_json(&event_content)?;
        
        let (matches, applicable) = tree.evaluate(&event).await;
        tracing::error!("Match: {}, Applicable: {}", matches, applicable);
    }
    
    Ok(())
}
```

### 9.2 Web API
```rust
// src/web.rs
use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct ParseRequest {
    rule: String,
    event: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ParseResponse {
    success: bool,
    matches: Option<bool>,
    errors: Vec<String>,
}

async fn parse_rule_handler(Json(req): Json<ParseRequest>) -> Json<ParseResponse> {
    match parse_rule(&req.rule).await {
        Ok(rule) => {
            if let Some(event_data) = req.event {
                // Evaluate against event
                match compile_condition(&rule).await {
                    Ok(tree) => {
                        let event = Event::from_value(event_data);
                        let (matches, _) = tree.evaluate(&event).await;
                        Json(ParseResponse {
                            success: true,
                            matches: Some(matches),
                            errors: vec![],
                        })
                    },
                    Err(e) => Json(ParseResponse {
                        success: false,
                        matches: None,
                        errors: vec![e.to_string()],
                    }),
                }
            } else {
                Json(ParseResponse {
                    success: true,
                    matches: None,
                    errors: vec![],
                })
            }
        },
        Err(e) => Json(ParseResponse {
            success: false,
            matches: None,
            errors: vec![e.to_string()],
        }),
    }
}

pub fn create_app() -> Router {
    Router::new()
        .route("/parse", post(parse_rule_handler))
}
```

## Phase 10: Documentation and Examples (Week 6)

### 10.1 API Documentation
```rust
//! # Sigma Rule Parser
//! 
//! A high-performance, async Sigma rule parser for Rust.
//! 
//! ## Quick Start
//! 
//! ```rust
//! use sigma_rs::{parse_rule, compile_condition, Event};
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let rule_yaml = r#"
//!         title: Example Rule
//!         detection:
//!             selection:
//!                 EventID: 4625
//!             condition: selection
//!     "#;
//!     
//!     let rule = parse_rule(rule_yaml).await?;
//!     let tree = compile_condition(&rule).await?;
//!     
//!     let event = Event::from_json(r#"{"EventID": 4625}"#)?;
//!     let (matches, applicable) = tree.evaluate(&event).await;
//!     
//!     tracing::error!("Match: {}", matches);
//!     Ok(())
//! }
//! ```
```

### 10.2 Examples
```rust
// examples/streaming_parser.rs
use futures::StreamExt;
use sigma_rs::{parse_rule_stream, Event};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rule_stream = parse_rule_stream("rules/*.yml").await?;
    
    tokio::pin!(rule_stream);
    
    while let Some(rule_result) = rule_stream.next().await {
        match rule_result {
            Ok(rule) => {
                tracing::error!("Parsed rule: {}", rule.title);
            },
            Err(e) => {
                tracing::error!("Error parsing rule: {}", e);
            }
        }
    }
    
    Ok(())
}
```

## Conclusion

This implementation plan provides a comprehensive approach to building a Sigma rule parser in Rust that:

1. Maintains compatibility with the Go implementation
2. Leverages Rust's async/await for performance
3. Provides robust error handling and recovery
4. Offers flexible APIs for various use cases
5. Includes extensive testing and benchmarking
6. Supports both CLI and web interfaces

The modular design allows for incremental development while ensuring each component can be tested independently. The use of async patterns enables efficient processing of large rule sets and concurrent evaluation of complex conditions.
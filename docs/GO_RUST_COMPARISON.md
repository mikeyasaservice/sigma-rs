# Comparison of Go and Rust Sigma Implementations

## Overview
This document provides a detailed comparison of the key structures and interfaces between the Go and Rust implementations of the Sigma rule engine.

## Event Interface/Trait

### Go Implementation (`sigma.go`)
```go
type Keyworder interface {
    Keywords() ([]string, bool)
}

type Selector interface {
    Select(string) (interface{}, bool)
}

type Event interface {
    Keyworder
    Selector
}
```

### Rust Implementation (`event.rs`)
```rust
pub trait Keyworder {
    fn keywords(&self) -> (Vec<String>, bool);
}

pub trait Selector {
    fn select(&self, key: &str) -> (Option<Value>, bool);
}

pub trait Event: Keyworder + Selector + Send + Sync {
    fn id(&self) -> &str;
    fn timestamp(&self) -> i64;
}
```

**Key Differences:**
- Rust uses `Option<Value>` instead of Go's `interface{}, bool` pattern
- Rust Event trait adds `id()` and `timestamp()` methods not present in Go
- Rust has a custom `Value` enum vs Go's `interface{}`
- Rust Event requires `Send + Sync` for thread safety
- Rust has additional `AsyncEvent` trait for async processing

## Pattern Matching

### Go Implementation (`pattern.go`)
```go
type TextPatternModifier int

const (
    TextPatternNone TextPatternModifier = iota
    TextPatternContains
    TextPatternPrefix
    TextPatternSuffix
    TextPatternAll
    TextPatternRegex
    TextPatternKeyword
)

type NumMatcher interface {
    NumMatch(int) bool
}

type StringMatcher interface {
    StringMatch(string) bool
}
```

### Rust Implementation (`pattern/mod.rs`)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPatternModifier {
    None,
    Contains,
    Prefix,
    Suffix,
    All,
    Regex,
    Keyword,
}

pub trait NumMatcher: Send + Sync {
    fn num_match(&self, val: i64) -> bool;
}

pub trait StringMatcher: Send + Sync {
    fn string_match(&self, s: &str) -> bool;
}
```

**Key Differences:**
- Rust uses proper enums vs Go's `iota` constants
- Rust traits require `Send + Sync`
- Rust uses `i64` for numbers vs Go's `int`
- Better type safety in Rust with derive macros

## Tree/AST Structure

### Go Implementation (`tree.go`, `nodes.go`)
```go
type Tree struct {
    Root Branch
    Rule *RuleHandle
}

type Matcher interface {
    Match(Event) (bool, bool)
}

type Branch interface {
    Matcher
}

// Node types
type NodeSimpleAnd []Branch
type NodeSimpleOr []Branch
type NodeNot struct { B Branch }
type NodeAnd struct { L, R Branch }
type NodeOr struct { L, R Branch }
```

### Rust Implementation (`tree/mod.rs`, `ast/nodes.rs`)
```rust
pub struct Tree {
    pub root: Arc<dyn Branch>,
    pub rule: Arc<RuleHandle>,
}

#[async_trait]
pub trait Branch: Debug + Send + Sync {
    async fn matches(&self, event: &dyn Event) -> MatchResult;
    fn describe(&self) -> String;
}

// Node types with Arc for thread safety
pub struct NodeAnd {
    pub left: Arc<dyn Branch>,
    pub right: Arc<dyn Branch>,
}
// Similar for NodeOr, NodeNot, NodeSimpleAnd, NodeSimpleOr
```

**Key Differences:**
- Rust uses `Arc<dyn Branch>` for thread-safe reference counting
- Rust has async matching with `async_trait`
- Rust has explicit `MatchResult` struct vs Go's `(bool, bool)` tuple
- Rust requires `describe()` method for debugging
- More explicit lifetime and ownership management in Rust

## Result Structure

### Go Implementation (`rule.go`)
```go
type Result struct {
    Tags        `json:"tags"`
    ID          string `json:"id"`
    Title       string `json:"title"`
    Description string `json:"description"`
}

type Results []Result
```

### Rust Implementation (`result/mod.rs`)
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Result {
    #[serde(default)]
    pub tags: Vec<String>,
    pub id: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Results(pub Vec<Result>);
```

**Key Differences:**
- Rust has builder pattern methods (`new()`, `with_tags()`)
- Rust Results is a newtype wrapper with methods
- Better iterator implementations in Rust
- More traits automatically derived in Rust

## Additional Features in Rust

### Features Present in Rust but Missing in Go:
1. **Async Support**: Comprehensive async/await support throughout
2. **Value Enum**: Strongly typed value system instead of `interface{}`
3. **Event Builder**: Builder pattern for constructing events
4. **Error Handling**: Uses `Result<T, E>` instead of tuple returns
5. **Consumer Module**: Advanced streaming and consumption framework
6. **Service Module**: Service-oriented architecture support
7. **Metrics**: Built-in metrics collection
8. **DLQ (Dead Letter Queue)**: Error handling for failed events
9. **Backpressure**: Flow control for event streams
10. **Better Testing**: Property-based testing, extensive test coverage

### Architectural Improvements:
1. **Thread Safety**: Explicit `Send + Sync` bounds
2. **Memory Management**: Arc-based reference counting
3. **Type Safety**: Stronger type system with enums
4. **Modularity**: Better module separation and organization
5. **Performance**: Zero-copy patterns, optimized string matching

## Migration Recommendations

To complete the Rust implementation and maintain Go compatibility:

1. **Event Trait**: Consider making `id()` and `timestamp()` optional methods
2. **Pattern Matching**: Ensure all Go pattern types are supported
3. **Tree Building**: Verify parser generates compatible AST structures
4. **Result Format**: Ensure JSON serialization matches Go format
5. **Testing**: Add compatibility tests against Go implementation

## Summary

The Rust implementation represents a significant architectural improvement over the Go version while maintaining core compatibility. Key advantages include:
- Better type safety and error handling
- Thread safety by design
- Async support throughout
- More modular and extensible architecture
- Performance optimizations
- Richer feature set for production use

The main areas needing attention are:
- Complete feature parity testing
- Documentation updates
- Migration guides for Go users
- Performance benchmarking
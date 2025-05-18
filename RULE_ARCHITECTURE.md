# Rule and Ruleset Architecture Plan

## Overview
The rule system in Sigma consists of several key components:
1. Rule structure (metadata + detection logic)
2. Tree structure (AST representation of the detection logic)
3. Ruleset (collection of rules with evaluation capabilities)
4. Result types for positive matches

## Key Components

### Rule Structure
```go
type Rule struct {
    Author         string
    Description    string
    Falsepositives []string
    Fields         []string
    ID             string
    Level          string
    Title          string
    Status         string
    References     []string
    Logsource      Logsource
    Detection      Detection
    Tags           Tags
}
```

### RuleHandle
Extends Rule with metadata about the file source:
```go
type RuleHandle struct {
    Rule
    Path         string
    Multipart    bool
    NoCollapseWS bool
}
```

### Detection
The Detection type is a map with special handling for the "condition" key:
```go
type Detection map[string]interface{}

// Extract returns all non-condition keys
func (d Detection) Extract() map[string]interface{}
```

### Tree
The Tree structure combines a parsed AST with rule metadata:
```go
type Tree struct {
    Root Branch
    Rule *RuleHandle
}
```

### Ruleset
A thread-safe collection of parsed rules:
```go
type Ruleset struct {
    mu *sync.RWMutex
    Rules []*Tree
    root  []string
    Total, Ok, Failed, Unsupported int
}
```

## Core Workflows

### Rule Loading
1. Find all YAML files in specified directories
2. Parse YAML into Rule structs
3. Create RuleHandle with file metadata
4. Build Tree (AST) from Rule
5. Collect Trees into Ruleset

### Rule Evaluation
1. `Ruleset.EvalAll(Event)` evaluates all rules against an event
2. Each Tree evaluates using `Tree.Eval(Event)`
3. Tree delegates to AST root node for matching
4. Results are collected for positive matches

### AST Building
1. Extract condition string from Detection
2. Lex the condition into tokens
3. Parse tokens into AST nodes
4. Create Tree with root Branch

## Rust Implementation Plan

### Module Structure
```
src/
├── rule/
│   ├── mod.rs          # Rule and RuleHandle types
│   ├── detection.rs    # Detection type
│   ├── logsource.rs    # Logsource type
│   └── tags.rs         # Tags type
├── tree/
│   ├── mod.rs          # Tree struct and impl
│   └── builder.rs      # AST building logic
├── ruleset/
│   ├── mod.rs          # Ruleset struct and impl
│   ├── config.rs       # Config type
│   └── loader.rs       # File loading utilities
└── result/
    └── mod.rs          # Result and Results types
```

### Key Traits and Types

```rust
// In rule/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub author: Option<String>,
    pub description: Option<String>,
    pub falsepositives: Vec<String>,
    pub fields: Vec<String>,
    pub id: String,
    pub level: Option<String>,
    pub title: String,
    pub status: Option<String>,
    pub references: Vec<String>,
    pub logsource: Logsource,
    pub detection: Detection,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RuleHandle {
    pub rule: Rule,
    pub path: PathBuf,
    pub multipart: bool,
    pub no_collapse_ws: bool,
}

// In tree/mod.rs
pub struct Tree {
    pub root: Arc<dyn Branch>,
    pub rule: Arc<RuleHandle>,
}

// In ruleset/mod.rs
pub struct Ruleset {
    rules: Arc<RwLock<Vec<Arc<Tree>>>>,
    root: Vec<PathBuf>,
    stats: Arc<RwLock<RulesetStats>>,
}
```

### Dependencies
- `serde` and `serde_yaml` for YAML parsing
- `glob` crate for pattern matching
- `walkdir` for recursive directory scanning
- Existing AST nodes from our parser implementation

### Thread Safety
- Use `Arc<RwLock<>>` for thread-safe access to ruleset
- Immutable rule structures once parsed
- Share Tree instances across threads using Arc

### Error Handling
Mirror the Go error types:
- Parse errors (YAML, AST)
- Missing field errors
- Unsupported feature errors
- Bulk error collection for resilient loading

## Next Steps
1. Implement rule module with YAML deserialization
2. Port tree building logic integrating with existing parser
3. Implement ruleset with thread-safe evaluation
4. Add comprehensive tests
5. Benchmark against Go implementation
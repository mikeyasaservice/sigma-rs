# Sigma Rule Engine Implementation Analysis

## Core Architecture Comparison

### Go Implementation Structure
```
Event Interface (Keyworder + Selector)
    ↓
Pattern Matchers (StringMatcher, NumMatcher)
    ↓
Identifiers (Keyword, Selection)
    ↓
AST Nodes (NodeAnd, NodeOr, NodeNot, etc.)
    ↓
Tree (Root Branch + Rule Handle)
    ↓
Result
```

### Rust Implementation Structure
```
Event Trait (Keyworder + Selector + ID/Timestamp)
    ↓
Pattern Traits (StringMatcher, NumMatcher) with Send+Sync
    ↓
Field Rules (FieldPattern with String/Numeric/Keywords)
    ↓
AST Nodes (Arc-based thread-safe nodes)
    ↓
Tree (Arc<dyn Branch> + Arc<RuleHandle>)
    ↓
Result with Builder Pattern
```

## Key Implementation Details

### 1. Identifier Types (Go `ident.go`)
Go has two main identifier types:
- **Keywords**: Simple string matching against event keywords
- **Selection**: Field-based matching with key-value pairs

The Rust implementation needs to map these to:
- `FieldPattern::Keywords` for keyword matching
- `FieldPattern::String` or `FieldPattern::Numeric` for selection matching

### 2. Pattern Modifiers
Both implementations support:
- None (literal matching)
- Contains (wildcards on both sides)
- Prefix (startswith)
- Suffix (endswith)
- Regex (regular expressions)
- Keyword (for keyword identifiers)
- All (conjunction of patterns)

### 3. Value Type Handling
Go uses extensive type switching for numeric values:
- string → int conversion
- json.Number handling
- float64, int, int64, etc.

Rust uses the `Value` enum:
- String, Integer, Float, Boolean, Array, Object, Null
- More type-safe approach with explicit conversions

### 4. Threading Model
- **Go**: No explicit thread safety requirements
- **Rust**: Everything is `Send + Sync` for thread safety, uses `Arc` for shared ownership

## Critical Implementation Requirements

### 1. Identifier Type Detection
The Go `checkIdentType()` function needs equivalent in Rust:
```go
func checkIdentType(name string, data interface{}) identType {
    if strings.HasPrefix(name, "keyword") {
        return identKeyword
    }
    return reflectIdentKind(data)
}
```

### 2. Selection Field Key Processing
Go supports modifiers in field keys using `|` separator:
- `fieldname|contains`
- `fieldname|startswith`
- `fieldname|endswith`
- `fieldname|re`
- `fieldname|all`

Rust needs to parse these modifiers when building `FieldRule`.

### 3. Whitespace Handling
Go has global `gWSCollapse` regex and `handleWhitespace()` function.
Rust needs equivalent implementation for the `no_collapse_ws` flag.

### 4. Type Mismatch Handling
Go tracks statistics for type mismatches. Rust could add:
- Error types for type mismatches
- Optional metrics collection

## Missing Implementations in Rust

1. **Keyword Identifier**: Needs implementation of keyword-type identifiers
2. **Selection Modifiers**: Parse `|` separated modifiers in field keys
3. **Glob Escaping**: Port `escapeSigmaForGlob()` function
4. **Multiple Pattern Types**: Support arrays of patterns in selections
5. **Type Coercion**: Handle numeric type conversions like Go

## Architectural Advantages in Rust

1. **Type Safety**: Enum-based value system vs interface{}
2. **Async Support**: Native async/await for event processing
3. **Error Handling**: Result<T, E> vs tuple returns
4. **Memory Safety**: Arc-based reference counting
5. **Performance**: Zero-copy optimizations possible

## Implementation Roadmap

### Phase 1: Core Compatibility
1. Implement identifier type detection
2. Add selection field modifiers
3. Port whitespace handling
4. Add glob escape logic

### Phase 2: Type System
1. Implement type coercion for numeric values
2. Add comprehensive Value conversions
3. Handle all Go numeric types

### Phase 3: Advanced Features
1. Add metrics collection
2. Implement consumer framework
3. Add service architecture

### Phase 4: Testing & Validation
1. Compatibility test suite
2. Performance benchmarks
3. Rule validation tools
4. Migration utilities

## Testing Strategy

1. **Unit Tests**: Each component matches Go behavior
2. **Integration Tests**: Full rule evaluation matches Go
3. **Compatibility Tests**: Real Sigma rules produce same results
4. **Performance Tests**: Benchmark against Go implementation
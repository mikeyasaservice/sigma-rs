# Pattern Matching Architecture

## Overview

The pattern matching system in sigma-rs is responsible for matching field values against patterns defined in Sigma rules. The system implements various matching strategies including exact matching, glob patterns, regular expressions, and keyword matching.

## Key Components

### 1. Pattern Types

#### TextPatternModifier
- **None**: Direct matching
- **Contains**: Adds wildcards around pattern (*pattern*)
- **Prefix**: Match at beginning of string
- **Suffix**: Match at end of string
- **All**: All patterns must match (conjunction)
- **Regex**: Regular expression matching
- **Keyword**: Keyword-based matching (treated as contains)

#### FieldPattern
Basic matching patterns for field values:
- **Exact**: Exact string/value match
- **Glob**: Glob pattern matching (with * and ? wildcards)
- **Regex**: Regular expression matching
- **Keywords**: Match against event keywords

### 2. Matcher Traits

#### StringMatcher
Trait for all string pattern matchers:
```rust
pub trait StringMatcher: Send + Sync {
    fn string_match(&self, value: &str) -> bool;
}
```

#### NumMatcher
Trait for numeric pattern matchers:
```rust
pub trait NumMatcher: Send + Sync {
    fn num_match(&self, value: i64) -> bool;
}
```

### 3. Pattern Implementations

#### String Patterns
- **ContentPattern**: Exact content matching
- **PrefixPattern**: String prefix matching
- **SuffixPattern**: String suffix matching
- **GlobPattern**: Glob pattern matching
- **RegexPattern**: Regular expression matching

#### Numeric Patterns
- **NumPattern**: Exact numeric value matching
- **NumRange**: Numeric range matching (future)

### 4. Pattern Factories

- **new_string_matcher**: Creates appropriate string matcher based on pattern and modifiers
- **new_num_matcher**: Creates numeric matchers from value lists

### 5. Optimization

Pattern matchers can be optimized for performance:
- Literals evaluated before globs
- Globs evaluated before regular expressions
- Reusable pattern objects (compiled regex, glob patterns)

### 6. Special Features

#### Whitespace Handling
- Optional whitespace collapsing for non-regex patterns
- Configurable through `no_collapse_ws` flag

#### Sigma Escape Rules
Special handling for Sigma escape sequences:
- Single backslash for literals
- Escaped wildcards (\*)
- Backslash before wildcards (\\*)
- Complex escape sequences

## Design Decisions

1. **Trait-based Design**: Using traits allows for extensible matcher types
2. **Async Support**: Matchers implement Send + Sync for async contexts
3. **Optimization**: Performance-critical path optimizations through pattern ordering
4. **Reusability**: Compiled patterns (regex, glob) are reused across matches
5. **Flexibility**: Support for various Sigma pattern types and modifiers
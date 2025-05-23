# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Sigma-rs is a high-performance Rust implementation of the Sigma rule engine for real-time security event processing, designed for production use with Redpanda/Kafka integration and comprehensive performance optimizations.

## Common Development Commands

### Build & Test
```bash
# Build with all features
cargo build --all-features

# Run tests with all features
cargo test --all-features

# Run comprehensive test suite with coverage
./scripts/test_all.sh --coverage

# Run only unit tests
cargo test --lib

# Run specific test
cargo test test_name

# Run benchmarks
cargo bench

# Quick benchmark
cargo bench -- --quick
```

### Code Quality
```bash
# Format code
cargo fmt

# Lint with strict warnings
cargo clippy -- -D warnings

# Run coverage analysis (85% threshold)
./scripts/coverage.sh

# Security audit
cargo audit

# Run compatibility tests against Go implementation
./scripts/compatibility_test.sh
```

### Feature Flags
- `kafka` - Redpanda/Kafka consumer support
- `metrics` - Prometheus metrics integration  
- `service` - HTTP/gRPC service layer
- `all` - Enable all features (use `--all-features` flag)

## Architecture Overview

### Core Processing Pipeline
1. **Rule Parsing** (`rule/`) - YAML rule loading and validation
2. **AST Construction** (`ast/`) - Abstract syntax tree for rule conditions
3. **Lexical Analysis** (`lexer/`) - Tokenization of rule expressions
4. **Parser** (`parser/`) - Condition parsing with comprehensive validation
5. **Pattern Matching** (`pattern/`) - String/numeric matchers with escape handling
6. **Tree Building** (`tree/`) - Detection tree construction and optimization
7. **Event Processing** (`event/`) - Field selection and event abstraction
8. **Matching Engine** (`matcher/`) - Core rule evaluation with lazy evaluation

### Production Components
- **Consumer** (`consumer/`) - Production Kafka consumer with manual offset management, DLQ support, backpressure control, exponential backoff retry
- **Aggregation** (`aggregation/`) - Sliding window aggregation for time-based rules
- **Service** (`service/`) - Optional HTTP/gRPC service layer
- **Engine** (`engine.rs`) - Main orchestration and lifecycle management

### Key Design Patterns
- **Performance**: Aho-Corasick string matching, lazy evaluation, parallel processing
- **Reliability**: Comprehensive error handling, graceful shutdown, circuit breakers
- **Observability**: Structured logging, Prometheus metrics, health checks
- **Safety**: `#![deny(unsafe_code)]` - no unsafe code allowed

### Testing Strategy
- **Unit Tests**: Per-module with extensive edge case coverage
- **Integration Tests**: End-to-end rule processing workflows
- **Property Tests**: Fuzz testing with proptest for parser/lexer
- **Compatibility Tests**: Parity validation against Go reference implementation
- **Performance Tests**: Comprehensive benchmark suite with historical tracking

## Examples Usage

Run examples to understand usage patterns:
```bash
# Validate rules in parallel
cargo run --example rule_validator --all-features

# Process JSON events
cargo run --example event_detector --all-features

# Real-time stream processing
cargo run --example stream_detector --all-features
```

## CI/CD Integration

The project uses comprehensive GitHub Actions workflows:
- Multi-platform testing (Ubuntu, Windows, macOS)
- Multiple Rust versions (stable, beta, nightly)
- All feature combination testing
- Automated security auditing and performance benchmarking
- Pre-commit hooks for code quality enforcement
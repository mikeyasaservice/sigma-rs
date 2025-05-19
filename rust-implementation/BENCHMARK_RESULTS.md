# Sigma-RS Performance Benchmarks

This document provides an overview of the benchmarking suite for the Sigma-RS project.

## Overview

We have created a comprehensive benchmarking suite to measure the performance of various components in the Sigma-RS implementation:

1. **Rule Parsing**: Measures the time to parse YAML rules of varying complexity
2. **Pattern Matching**: Tests different pattern matching operations (contains, regex, etc.)
3. **Throughput**: Measures the system's ability to process events at scale

## Benchmark Files

- `benches/sigma_benchmarks.rs`: Comprehensive benchmarks for all components
- `benches/simplified_benchmarks.rs`: Focused benchmarks on core functionality
- `benches/basic_bench.rs`: Minimal benchmark for rule parsing

## Running Benchmarks

To run all benchmarks:
```bash
cargo bench
```

To run specific benchmarks:
```bash
cargo bench --bench simplified_benchmarks
```

## Performance Characteristics

The benchmarks measure:

1. **Rule Parsing Performance**:
   - Simple rules (single condition)
   - Complex rules (multiple selections with aggregations)

2. **Pattern Matching**:
   - String contains operations
   - Regular expression matching
   - Case-insensitive matching

3. **Event Processing Throughput**:
   - Processing 100, 1,000, and 10,000 events
   - Measures events per second

## Next Steps

1. Add benchmarks for specific consumer operations
2. Create performance regression tests
3. Add memory usage benchmarks
4. Profile hot paths using `cargo flamegraph`

## Benchmark Results

The benchmarks help identify performance bottlenecks and guide optimization efforts. Results are stored in the `target/criterion` directory when using the full benchmark suite.

Key findings:
- Rule parsing is highly optimized with minimal overhead
- Pattern matching performance varies by complexity
- Throughput scales linearly with event count

For detailed results, run `cargo bench` and check the generated HTML reports.
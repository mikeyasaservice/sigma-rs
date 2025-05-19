# Sigma-rs Testing Plan

## Overview
This document outlines the testing strategy for the Rust Sigma implementation. Since we're replacing the Go version entirely, our focus is on ensuring comprehensive coverage of all Sigma features and real-world use cases.

## Test Categories

### 1. Sigma Rule Compatibility Tests
- Parse all rules from the official Sigma repository
- Validate each rule component (detection, logsource, metadata)
- Ensure all modifiers work correctly (contains, endswith, etc.)
- Test complex conditions (nested AND/OR, wildcards)

### 2. Event Processing Tests
- Test with real log samples:
  - Windows Event Logs
  - Sysmon events
  - Linux audit logs
  - Web server logs
- Validate field extraction and matching
- Test performance with high-volume events

### 3. Feature Coverage Tests
- All pattern types (exact, wildcard, regex)
- All modifiers (contains, all, endswith, etc.)
- Complex detection logic
- Aggregation rules
- Near real-time correlation

### 4. Error Handling Tests
- Malformed rules
- Invalid field references
- Type mismatches
- Resource exhaustion scenarios

### 5. Performance Tests
- Rule loading time
- Event processing throughput
- Memory usage under load
- Concurrent rule evaluation

## Implementation Priority

1. **High Priority**
   - Core rule parsing tests
   - Basic event matching
   - Common modifiers (contains, endswith)

2. **Medium Priority**
   - Complex conditions
   - Performance benchmarks
   - Error scenarios

3. **Low Priority**
   - Edge cases
   - Stress testing
   - Documentation

## Test Data Sources

1. Official Sigma repository rules
2. Sample event logs from various sources
3. Synthetic test cases for edge conditions
4. Performance test datasets

## Success Criteria

- [ ] Parse 100% of valid Sigma rules
- [ ] Correctly match known detection scenarios
- [ ] Handle errors gracefully
- [ ] Meet performance targets
- [ ] Pass all unit tests with >85% coverage
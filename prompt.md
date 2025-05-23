# Consistent Code Review Prompt for AI Analysis

    ## Objective
    Provide a **systematic, reproducible code review** that generates consistent results across multiple analysis 
    sessions.

    ## Review Methodology

    ### Phase 1: Systematic Code Inventory
    **Before any analysis, create a complete inventory:**

    1. **File Structure Mapping**
       - List all source files with line counts
       - Identify entry points and main execution paths  
       - Map module dependencies and relationships

    2. **Pattern Recognition Baseline**
       - Count total functions, structs, traits, and implementations
       - Identify all `unsafe` blocks and external dependencies
       - List all async functions and their await points

    ### Phase 2: Checklist-Driven Analysis
    **Use this exact checklist in order, documenting findings for each item:**

    #### Security Review Checklist
    - [ ] **Input Validation**: Search for all `serde::Deserialize` implementations - are bounds checked?
    - [ ] **Error Handling**: Count `unwrap()`, `expect()`, and `panic!` calls - document each location
    - [ ] **Unsafe Code**: List every `unsafe` block with justification review
    - [ ] **Secrets Management**: Search for hardcoded strings that could be credentials
    - [ ] **External Calls**: Identify all network calls and external process invocations

    #### Performance Review Checklist  
    - [ ] **Memory Allocations**: Count `Vec::new()`, `String::new()`, `HashMap::new()` in hot paths
    - [ ] **String Operations**: Identify all `.clone()`, `.to_string()`, `.to_owned()` calls
    - [ ] **Async Efficiency**: List all blocking operations inside async functions
    - [ ] **Collection Efficiency**: Check for O(n²) operations and unnecessary iterations

    #### Reliability Review Checklist
    - [ ] **Error Propagation**: Trace error paths from external boundaries to handlers
    - [ ] **Resource Cleanup**: Verify Drop implementations and RAII patterns
    - [ ] **Timeout Handling**: Check all network operations have timeouts
    - [ ] **Graceful Shutdown**: Verify signal handling and cleanup procedures

    #### Concurrency Review Checklist
    - [ ] **Shared State**: List all `Arc<Mutex<T>>` and `Arc<RwLock<T>>` usage
    - [ ] **Race Conditions**: Identify potential data races in async code
    - [ ] **Deadlock Prevention**: Check lock acquisition order consistency
    - [ ] **Channel Usage**: Verify proper channel sender/receiver lifecycle

    ### Phase 3: Quantitative Metrics
    **Generate exact counts for consistency:**

    ```
    Code Metrics:
    - Total Lines of Code: [COUNT]
    - Function Count: [COUNT] 
    - Async Function Count: [COUNT]
    - Test Function Count: [COUNT]
    - Unsafe Block Count: [COUNT]
    - TODO/FIXME Comments: [COUNT]
    - External Dependencies: [COUNT]
    ```

    ### Phase 4: Standardized Severity Classification

    **Use exact severity definitions:**

    #### 🔴 CRITICAL (Production Blocker)
    - Memory safety violations
    - Potential data corruption
    - Security vulnerabilities with external exposure
    - Unhandled error paths that could crash the service

    #### 🟠 HIGH (Fix Before Release)  
    - Performance bottlenecks in core paths
    - Resource leaks
    - Missing error handling for expected failures
    - Race conditions in concurrent code

    #### 🟡 MEDIUM (Next Sprint)
    - Code duplication > 10 lines
    - Missing documentation on public APIs
    - Inefficient algorithms with better alternatives
    - Technical debt affecting maintainability

    #### 🟢 LOW (Future Improvements)
    - Style inconsistencies
    - Minor performance optimizations
    - Enhanced error messages
    - Additional convenience methods

    ### Phase 5: Reproducible Analysis Framework

    **For each identified issue, provide:**

    1. **Exact Location**: File path, line number, function name
    2. **Code Snippet**: Exact problematic code (< 10 lines)
    3. **Root Cause**: Technical explanation of why it's problematic
    4. **Impact Assessment**: What could go wrong in production
    5. **Specific Fix**: Concrete code suggestion or architectural change
    6. **Verification Method**: How to test the fix works

    ## Consistency Controls

    ### Before Starting Analysis:
    - [ ] Confirm total number of Rust files to analyze
    - [ ] Set specific order for file analysis (alphabetical by module)
    - [ ] Define scope boundaries (exclude examples/, tests/ folders?)

    ### During Analysis:
    - [ ] Process files in declared order
    - [ ] Use identical search patterns for each category
    - [ ] Document "Not Found" results for negative findings
    - [ ] Maintain running count of issues by severity

    ### Output Validation:
    - [ ] Verify all claimed line numbers exist in uploaded files
    - [ ] Ensure no duplicate issues reported
    - [ ] Confirm all suggestions compile with provided context
    - [ ] Cross-reference findings with initial code inventory

    ## Response Format Template

    ```markdown
    # Code Review Results - [PROJECT_NAME]

    ## Analysis Summary
    - Files Analyzed: X
    - Total Issues Found: Y
    - Critical: A | High: B | Medium: C | Low: D

    ## Critical Issues [🔴 A found]
    ### Issue 1: [Specific Title]
    **Location**: `src/module.rs:123` in `function_name()`
    **Code**: 
    ```rust
    [exact problematic code]
    ```
    **Problem**: [technical explanation]
    **Fix**: [specific solution]
    **Test**: [verification method]

    [Repeat for each critical issue]

    ## Detailed Findings by Category
    [Continue with systematic breakdown]

    ## Production Readiness Score: X/10
    [Justification based on findings]
    ```

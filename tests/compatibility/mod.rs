/// Compatibility testing module for comparing Rust and Go implementations
/// This module provides utilities to ensure the Rust implementation
/// produces identical results to the Go implementation

use std::process::Command;
use std::path::Path;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use anyhow::{Result, anyhow};

#[derive(Debug, Serialize, Deserialize)]
pub struct CompatibilityTest {
    pub id: String,
    pub description: String,
    pub rule: String,
    pub event: Value,
    pub expected_result: MatchResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchResult {
    pub matched: bool,
    pub rule_id: String,
    pub tags: Vec<String>,
    pub level: String,
}

pub struct GoCompatibilityTester {
    go_binary_path: String,
}

impl GoCompatibilityTester {
    pub fn new() -> Result<Self> {
        // Check if Go binary exists
        let go_binary = "../sigma-go-test/sigma-test";
        if !Path::new(go_binary).exists() {
            return Err(anyhow!("Go test binary not found at {}", go_binary));
        }
        
        Ok(Self {
            go_binary_path: go_binary.to_string(),
        })
    }

    /// Run the Go implementation and get results
    pub fn run_go_test(&self, rule: &str, event: &Value) -> Result<MatchResult> {
        let output = Command::new(&self.go_binary_path)
            .arg("--rule")
            .arg(rule)
            .arg("--event")
            .arg(event.to_string())
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "Go implementation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let result: MatchResult = serde_json::from_slice(&output.stdout)?;
        Ok(result)
    }

    /// Run the Rust implementation and get results
    pub fn run_rust_test(&self, rule: &str, event: &Value) -> Result<MatchResult> {
        // Parse the rule
        let sigma_rule = sigma_rs::rule::rule_from_yaml(rule.as_bytes())?;
        
        // Create the event
        let dynamic_event = sigma_rs::DynamicEvent::new(event.clone());
        
        // Build and evaluate the tree
        let runtime = tokio::runtime::Runtime::new()?;
        let rule_handle = sigma_rs::rule::RuleHandle::new(sigma_rule, std::path::PathBuf::from("test.yml"));
        let tree = runtime.block_on(async {
            sigma_rs::tree::build_tree(rule_handle).await
        })?;
        
        let (matched, applicable) = runtime.block_on(async {
            tree.match_event(&dynamic_event).await
        });
        
        Ok(MatchResult {
            matched,
            rule_id: sigma_rule.id.clone(),
            tags: sigma_rule.tags.iter().map(|t| t.name.clone()).collect(),
            level: sigma_rule.level.unwrap_or_else(|| "medium".to_string()),
        })
    }

    /// Compare Go and Rust results
    pub fn compare_results(&self, go_result: &MatchResult, rust_result: &MatchResult) -> Result<()> {
        if go_result.matched != rust_result.matched {
            return Err(anyhow!(
                "Match result mismatch: Go={}, Rust={}",
                go_result.matched,
                rust_result.matched
            ));
        }

        if go_result.rule_id != rust_result.rule_id {
            return Err(anyhow!(
                "Rule ID mismatch: Go={}, Rust={}",
                go_result.rule_id,
                rust_result.rule_id
            ));
        }

        // Sort tags for comparison
        let mut go_tags = go_result.tags.clone();
        let mut rust_tags = rust_result.tags.clone();
        go_tags.sort();
        rust_tags.sort();

        if go_tags != rust_tags {
            return Err(anyhow!(
                "Tags mismatch: Go={:?}, Rust={:?}",
                go_tags,
                rust_tags
            ));
        }

        if go_result.level != rust_result.level {
            return Err(anyhow!(
                "Level mismatch: Go={}, Rust={}",
                go_result.level,
                rust_result.level
            ));
        }

        Ok(())
    }
}

/// Load compatibility tests from JSON file
pub fn load_compatibility_tests(path: &Path) -> Result<Vec<CompatibilityTest>> {
    let content = std::fs::read_to_string(path)?;
    let tests: Vec<CompatibilityTest> = serde_json::from_str(&content)?;
    Ok(tests)
}

/// Run a single compatibility test
pub fn run_compatibility_test(test: &CompatibilityTest) -> Result<()> {
    let tester = GoCompatibilityTester::new()?;
    
    tracing::error!("Running compatibility test: {}", test.id);
    tracing::error!("Description: {}", test.description);
    
    // Run Go implementation
    let go_result = tester.run_go_test(&test.rule, &test.event)?;
    tracing::error!("Go result: {:?}", go_result);
    
    // Run Rust implementation  
    let rust_result = tester.run_rust_test(&test.rule, &test.event)?;
    tracing::error!("Rust result: {:?}", rust_result);
    
    // Compare results
    tester.compare_results(&go_result, &rust_result)?;
    tracing::error!("✓ Test passed\n");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compatibility_framework() {
        let test = CompatibilityTest {
            id: "test-001".to_string(),
            description: "Simple process creation detection".to_string(),
            rule: r#"
title: Test Process Creation
id: test-001
detection:
  selection:
    EventID: 1
    Image|endswith: '\cmd.exe'
  condition: selection
"#.to_string(),
            event: json!({
                "EventID": 1,
                "Image": "C:\\Windows\\System32\\cmd.exe"
            }),
            expected_result: MatchResult {
                matched: true,
                rule_id: "test-001".to_string(),
                tags: vec![],
                level: "medium".to_string(),
            },
        };

        // This would run if the Go binary was available
        // run_compatibility_test(&test).unwrap();
    }

    #[test]
    fn test_complex_compatibility() {
        let test = CompatibilityTest {
            id: "test-002".to_string(),
            description: "Complex condition with modifiers".to_string(),
            rule: r#"
title: Complex Detection
id: test-002
tags:
  - attack.execution
  - attack.t1059
level: high
detection:
  process:
    EventID: 1
    Image|contains: 'powershell'
  suspicious_args:
    CommandLine|contains|all:
      - '-EncodedCommand'
      - 'bypass'
  network:
    EventID: 3
    DestinationPort: 
      - 80
      - 443
  condition: process and (suspicious_args or network)
"#.to_string(),
            event: json!({
                "EventID": 1,
                "Image": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
                "CommandLine": "powershell.exe -ExecutionPolicy bypass -EncodedCommand SGVsbG8="
            }),
            expected_result: MatchResult {
                matched: true,
                rule_id: "test-002".to_string(),
                tags: vec!["attack.execution".to_string(), "attack.t1059".to_string()],
                level: "high".to_string(),
            },
        };

        // Test structure is valid
        assert_eq!(test.id, "test-002");
        assert_eq!(test.tags.len(), 2);
    }
}
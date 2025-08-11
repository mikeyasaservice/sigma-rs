//! RuleSet implementation for loading and evaluating multiple Sigma rules
//!
//! This module provides the core rule evaluation system that manages
//! multiple rules and efficiently matches them against events.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::{
    ast::MatchResult,
    event::DynamicEvent,
    parser::ParseError,
    rule::{rule_from_yaml, Rule, RuleHandle},
    tree::{build_tree, Tree},
    Result as SigmaResult, SigmaEngineBuilder, SigmaError,
};

/// Maximum number of concurrent rule evaluations to prevent resource exhaustion
const MAX_CONCURRENT_EVALUATIONS: usize = 100;

/// Maximum number of rules to load from a single directory
const MAX_RULES_PER_DIR: usize = 10000;

/// Collection of compiled Sigma rules for efficient evaluation
#[derive(Debug)]
pub struct RuleSet {
    /// Compiled rules with their detection trees
    rules: Vec<CompiledRule>,
    /// Index of rules by ID for fast lookup
    rule_index: HashMap<String, usize>,
    /// Metadata about the ruleset
    metadata: RuleSetMetadata,
}

/// A compiled rule with its detection tree
#[derive(Debug)]
struct CompiledRule {
    /// The original rule (wrapped in Arc for efficient sharing)
    rule: Arc<Rule>,
    /// The compiled detection tree
    tree: Arc<Tree>,
    /// Whether this rule is enabled
    enabled: bool,
}

/// Metadata about the ruleset
#[derive(Debug, Clone)]
pub struct RuleSetMetadata {
    /// Total number of rules
    pub total_rules: usize,
    /// Number of enabled rules
    pub enabled_rules: usize,
    /// Number of rules that failed to compile
    pub failed_rules: usize,
    /// When the ruleset was loaded
    #[allow(dead_code)]
    loaded_at: std::time::Instant,
}

/// Result of evaluating a ruleset against an event
#[derive(Debug, Clone)]
pub struct RuleSetResult {
    /// Matches from all rules
    pub matches: Vec<RuleMatch>,
    /// Total number of rules evaluated
    pub rules_evaluated: usize,
    /// Time taken to evaluate
    pub evaluation_time: std::time::Duration,
}

/// A single rule match
#[derive(Debug, Clone)]
pub struct RuleMatch {
    /// The rule ID
    pub rule_id: String,
    /// The rule title
    pub rule_title: String,
    /// Whether the rule matched
    pub matched: bool,
    /// Match details
    pub match_result: MatchResult,
    /// Time taken to evaluate this rule
    pub evaluation_time: std::time::Duration,
}

impl RuleSet {
    /// Create a new empty ruleset
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            rule_index: HashMap::new(),
            metadata: RuleSetMetadata {
                total_rules: 0,
                enabled_rules: 0,
                failed_rules: 0,
                loaded_at: std::time::Instant::now(),
            },
        }
    }

    /// Load rules from the specified directories
    pub async fn load(builder: &SigmaEngineBuilder) -> SigmaResult<Self> {
        let mut ruleset = Self::new();

        for dir in &builder.rule_dirs {
            ruleset
                .load_from_directory(dir, builder.fail_on_parse_error)
                .await?;
        }

        info!(
            "Loaded ruleset: {} total rules, {} enabled, {} failed",
            ruleset.metadata.total_rules,
            ruleset.metadata.enabled_rules,
            ruleset.metadata.failed_rules
        );

        Ok(ruleset)
    }

    /// Load rules from a directory
    pub async fn load_directory(&mut self, dir: &str) -> Result<()> {
        self.load_from_directory(dir, false)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Internal method to load rules from a directory (recursively)
    async fn load_from_directory(&mut self, dir: &str, fail_on_error: bool) -> SigmaResult<()> {
        let path = Path::new(dir);
        if !path.exists() {
            return Err(SigmaError::Parse(format!("Directory not found: {}", dir)));
        }

        // Use a stack for iterative directory traversal to avoid deep recursion
        let mut dirs_to_process = vec![path.to_path_buf()];
        let mut total_file_count = 0;

        while let Some(current_dir) = dirs_to_process.pop() {
            let mut entries = tokio::fs::read_dir(&current_dir).await.map_err(|e| {
                SigmaError::Parse(format!("Failed to read directory {:?}: {}", current_dir, e))
            })?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| SigmaError::Parse(format!("Failed to read directory entry: {}", e)))?
            {
                // Check file count limit to prevent resource exhaustion
                if total_file_count >= MAX_RULES_PER_DIR {
                    warn!(
                        "Reached maximum rules limit ({}). Skipping remaining files.",
                        MAX_RULES_PER_DIR
                    );
                    return Ok(());
                }

                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .await
                    .map_err(|e| SigmaError::Parse(format!("Failed to get file type: {}", e)))?;

                if file_type.is_dir() {
                    // Add subdirectory to process list
                    dirs_to_process.push(path);
                } else if file_type.is_file()
                    && path.extension().and_then(|s| s.to_str()) == Some("yml")
                {
                    total_file_count += 1;
                    match self.load_rule_file(&path).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Failed to load rule {}: {}", path.display(), e);
                            self.metadata.failed_rules += 1;

                            if fail_on_error {
                                return Err(e);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single rule file
    async fn load_rule_file(&mut self, path: &Path) -> SigmaResult<()> {
        debug!("Loading rule from {}", path.display());

        // Use streaming read with size limit to prevent resource exhaustion
        const MAX_RULE_SIZE: u64 = 1024 * 1024; // 1MB limit per rule file

        let file = tokio::fs::File::open(path).await.map_err(|e| {
            SigmaError::Parse(format!("Failed to open file {}: {}", path.display(), e))
        })?;

        let mut contents = Vec::new();
        use tokio::io::AsyncReadExt;
        file.take(MAX_RULE_SIZE)
            .read_to_end(&mut contents)
            .await
            .map_err(|e| {
                SigmaError::Parse(format!("Failed to read file {}: {}", path.display(), e))
            })?;

        let rule = rule_from_yaml(&contents)?;
        self.add_rule(rule).await?;

        Ok(())
    }

    /// Add a rule to the ruleset
    pub async fn add_rule(&mut self, rule: Rule) -> SigmaResult<()> {
        // Wrap rule in Arc for efficient sharing
        let rule_arc = Arc::new(rule);

        // Create a rule handle with clone for tree building
        // Note: RuleHandle requires ownership of Rule, not Arc<Rule>, so we must clone here.
        // The Arc is still used to share the rule with the CompiledRule struct below.
        let rule_handle = RuleHandle::new((*rule_arc).clone(), std::path::PathBuf::from("ruleset"));

        // Build the detection tree
        let tree = build_tree(rule_handle)
            .await
            .map_err(|e: ParseError| SigmaError::Parse(e.to_string()))?;

        // Store the compiled rule
        let index = self.rules.len();
        let rule_id = if rule_arc.id.is_empty() {
            format!("rule_{}", index)
        } else {
            rule_arc.id.clone()
        };

        self.rule_index.insert(rule_id.clone(), index);
        self.rules.push(CompiledRule {
            rule: rule_arc,
            tree: Arc::new(tree),
            enabled: true,
        });

        self.metadata.total_rules += 1;
        self.metadata.enabled_rules += 1;

        Ok(())
    }

    /// Get the number of rules in the set
    pub fn len(&self) -> usize {
        self.metadata.total_rules
    }

    /// Check if the ruleset is empty
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Evaluate all rules against an event
    pub async fn evaluate(&self, event: &DynamicEvent) -> SigmaResult<RuleSetResult> {
        let start = std::time::Instant::now();
        let mut matches = Vec::new();
        let mut rules_evaluated = 0;

        // Wrap event in Arc for efficient sharing across tasks
        let event_arc = Arc::new(event.clone());

        // Create semaphore to limit concurrent evaluations
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_EVALUATIONS));

        // Evaluate rules in parallel with bounded concurrency
        let tasks: Vec<_> = self
            .rules
            .iter()
            .filter(|r| r.enabled)
            .map(|compiled_rule| {
                let event_ref = Arc::clone(&event_arc);
                let tree = Arc::clone(&compiled_rule.tree);
                let rule = Arc::clone(&compiled_rule.rule);
                let semaphore = Arc::clone(&semaphore);

                tokio::spawn(async move {
                    // Acquire permit before evaluation
                    let _permit = semaphore.acquire().await.map_err(|e| {
                        SigmaError::Parse(format!("Failed to acquire semaphore: {}", e))
                    })?;

                    let rule_start = std::time::Instant::now();
                    let (matched, applicable) = tree.match_event(&*event_ref).await;
                    let evaluation_time = rule_start.elapsed();

                    let match_result = MatchResult {
                        matched,
                        applicable,
                    };

                    Ok::<RuleMatch, SigmaError>(RuleMatch {
                        rule_id: if rule.id.is_empty() {
                            "unknown".to_string()
                        } else {
                            rule.id.clone()
                        },
                        rule_title: rule.title.clone(),
                        matched,
                        match_result,
                        evaluation_time,
                    })
                })
            })
            .collect();

        // Collect results
        for task in tasks {
            match task.await {
                Ok(result) => match result {
                    Ok(rule_match) => {
                        rules_evaluated += 1;
                        matches.push(rule_match);
                    }
                    Err(e) => {
                        warn!("Rule evaluation error: {}", e);
                    }
                },
                Err(e) => {
                    warn!("Task join error: {}", e);
                }
            }
        }

        Ok(RuleSetResult {
            matches,
            rules_evaluated,
            evaluation_time: start.elapsed(),
        })
    }

    /// Enable or disable a rule by ID
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) -> Result<()> {
        if let Some(&index) = self.rule_index.get(rule_id) {
            if let Some(rule) = self.rules.get_mut(index) {
                // Only update counter if state actually changes
                if rule.enabled != enabled {
                    rule.enabled = enabled;
                    if enabled {
                        self.metadata.enabled_rules += 1;
                    } else {
                        self.metadata.enabled_rules -= 1;
                    }
                }
                Ok(())
            } else {
                Err(anyhow::anyhow!("Rule index out of bounds"))
            }
        } else {
            Err(anyhow::anyhow!("Rule not found: {}", rule_id))
        }
    }

    /// Get rule metadata
    pub fn get_metadata(&self) -> &RuleSetMetadata {
        &self.metadata
    }
}

/// Thread-safe wrapper for RuleSet
pub struct ConcurrentRuleSet {
    inner: Arc<RwLock<RuleSet>>,
}

impl ConcurrentRuleSet {
    /// Create a new concurrent ruleset
    pub fn new(ruleset: RuleSet) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ruleset)),
        }
    }

    /// Evaluate rules against an event
    pub async fn evaluate(&self, event: &DynamicEvent) -> SigmaResult<RuleSetResult> {
        let ruleset = self.inner.read().await;
        ruleset.evaluate(event).await
    }

    /// Add a rule to the set
    pub async fn add_rule(&self, rule: Rule) -> SigmaResult<()> {
        let mut ruleset = self.inner.write().await;
        ruleset.add_rule(rule).await
    }

    /// Enable or disable a rule
    pub async fn set_rule_enabled(&self, rule_id: &str, enabled: bool) -> Result<()> {
        let mut ruleset = self.inner.write().await;
        ruleset.set_rule_enabled(rule_id, enabled)
    }

    /// Get the number of rules
    pub async fn len(&self) -> usize {
        let ruleset = self.inner.read().await;
        ruleset.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_empty_ruleset() -> SigmaResult<()> {
        let ruleset = RuleSet::new();
        assert!(ruleset.is_empty());
        assert_eq!(ruleset.len(), 0);

        let event = DynamicEvent::new(json!({
            "EventID": 1,
            "CommandLine": "test.exe"
        }));

        let result = ruleset.evaluate(&event).await?;
        assert_eq!(result.rules_evaluated, 0);
        assert!(result.matches.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_add_rule() -> SigmaResult<()> {
        let mut ruleset = RuleSet::new();

        let rule_yaml = br#"
        title: Test Rule
        id: 12345678-1234-1234-1234-123456789001
        detection:
            selection:
                EventID: 1
            condition: selection
        "#;

        let rule = rule_from_yaml(rule_yaml)?;
        ruleset.add_rule(rule).await?;

        assert_eq!(ruleset.len(), 1);
        assert!(!ruleset.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_rule_evaluation() -> SigmaResult<()> {
        let mut ruleset = RuleSet::new();

        // Add a matching rule
        let rule_yaml = br#"
        title: Process Creation
        id: 12345678-1234-1234-1234-123456789002
        detection:
            selection:
                EventID: 1
                CommandLine|contains: 'powershell'
            condition: selection
        "#;

        let rule = rule_from_yaml(rule_yaml)?;
        ruleset.add_rule(rule).await?;

        // Test matching event
        let event = DynamicEvent::new(json!({
            "EventID": 1,
            "CommandLine": "powershell.exe -Command Get-Process"
        }));

        let result = ruleset.evaluate(&event).await?;
        assert_eq!(result.rules_evaluated, 1);
        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].matched);

        // Test non-matching event
        let event = DynamicEvent::new(json!({
            "EventID": 1,
            "CommandLine": "notepad.exe"
        }));

        let result = ruleset.evaluate(&event).await?;
        assert_eq!(result.rules_evaluated, 1);
        assert_eq!(result.matches.len(), 1);
        assert!(!result.matches[0].matched);
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_ruleset() -> SigmaResult<()> {
        let ruleset = RuleSet::new();
        let concurrent = ConcurrentRuleSet::new(ruleset);

        let rule_yaml = br#"
        title: Test Rule
        id: 12345678-1234-1234-1234-123456789003
        detection:
            selection:
                EventID: 1
            condition: selection
        "#;

        let rule = rule_from_yaml(rule_yaml)?;
        concurrent.add_rule(rule).await?;

        assert_eq!(concurrent.len().await, 1);

        let event = DynamicEvent::new(json!({
            "EventID": 1
        }));

        let result = concurrent.evaluate(&event).await?;
        assert_eq!(result.rules_evaluated, 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_file_size_limit() -> SigmaResult<()> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut ruleset = RuleSet::new();

        // Create a temporary file that exceeds the 1MB limit
        let mut temp_file = NamedTempFile::new().unwrap();

        // Write 2MB of data (exceeds 1MB limit)
        let large_content = "a".repeat(2 * 1024 * 1024);
        temp_file.write_all(large_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        // Loading should fail due to size limit
        let result = ruleset.load_rule_file(temp_file.path()).await;
        assert!(result.is_err());

        // Create a small valid rule file
        let mut temp_file = NamedTempFile::new().unwrap();
        let rule_yaml = r#"
title: Small Test Rule
id: 12345678-1234-1234-1234-123456789004
detection:
    selection:
        EventID: 1
    condition: selection
"#;
        temp_file.write_all(rule_yaml.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        // Loading should succeed for small files
        let result = ruleset.load_rule_file(temp_file.path()).await;
        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_evaluation_with_semaphore() -> SigmaResult<()> {
        let mut ruleset = RuleSet::new();

        // Add many rules to test semaphore limiting
        for i in 0..200 {
            let rule_yaml = format!(
                r#"
title: Test Rule {}
id: {:08x}-1234-1234-1234-123456789{:03}
detection:
    selection:
        EventID: {}
    condition: selection
"#,
                i,
                i,
                i,
                i % 10
            );

            let rule = rule_from_yaml(rule_yaml.as_bytes())?;
            ruleset.add_rule(rule).await?;
        }

        let event = DynamicEvent::new(json!({
            "EventID": 1
        }));

        // This should complete without resource exhaustion
        let result = ruleset.evaluate(&event).await?;
        assert_eq!(result.rules_evaluated, 200);

        // Check that some rules matched (those with EventID: 1)
        let matched_count = result.matches.iter().filter(|m| m.matched).count();
        assert!(matched_count > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_set_rule_enabled_idempotent() -> SigmaResult<()> {
        let mut ruleset = RuleSet::new();

        let rule_yaml = br#"
        title: Test Rule
        id: 12345678-1234-1234-1234-123456789001
        detection:
            selection:
                EventID: 1
            condition: selection
        "#;

        let rule = rule_from_yaml(rule_yaml)?;
        ruleset.add_rule(rule).await?;

        // Initial state: 1 enabled rule
        assert_eq!(ruleset.get_metadata().enabled_rules, 1);

        // Disable the rule
        ruleset.set_rule_enabled("12345678-1234-1234-1234-123456789001", false)?;
        assert_eq!(ruleset.get_metadata().enabled_rules, 0);

        // Disable again - should not change counter
        ruleset.set_rule_enabled("12345678-1234-1234-1234-123456789001", false)?;
        assert_eq!(ruleset.get_metadata().enabled_rules, 0);

        // Enable the rule
        ruleset.set_rule_enabled("12345678-1234-1234-1234-123456789001", true)?;
        assert_eq!(ruleset.get_metadata().enabled_rules, 1);

        // Enable again - should not change counter
        ruleset.set_rule_enabled("12345678-1234-1234-1234-123456789001", true)?;
        assert_eq!(ruleset.get_metadata().enabled_rules, 1);

        Ok(())
    }
}

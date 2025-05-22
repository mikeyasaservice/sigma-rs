//! RuleSet implementation for loading and evaluating multiple Sigma rules
//!
//! This module provides the core rule evaluation system that manages
//! multiple rules and efficiently matches them against events.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use anyhow::Result;

use crate::{
    rule::{Rule, rule_from_yaml, RuleHandle},
    tree::{Tree, build_tree},
    DynamicEvent,
    ast::MatchResult,
    event::adapter::AstEventAdapter,
    SigmaEngineBuilder,
    Result as SigmaResult,
    SigmaError,
    parser::ParseError,
};

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
    /// The original rule
    rule: Rule,
    /// The compiled detection tree
    tree: Arc<Tree>,
    /// Whether this rule is enabled
    enabled: bool,
}

/// Metadata about the ruleset
#[derive(Debug, Clone)]
pub struct RuleSetMetadata {
    /// Total number of rules
    total_rules: usize,
    /// Number of enabled rules
    enabled_rules: usize,
    /// Number of rules that failed to compile
    failed_rules: usize,
    /// When the ruleset was loaded
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
            ruleset.load_from_directory(dir, builder.fail_on_parse_error).await?;
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
        self.load_from_directory(dir, false).await.map_err(|e| anyhow::anyhow!(e))
    }
    
    /// Internal method to load rules from a directory
    async fn load_from_directory(&mut self, dir: &str, fail_on_error: bool) -> SigmaResult<()> {
        let path = Path::new(dir);
        if !path.exists() {
            return Err(SigmaError::Parse(format!("Directory not found: {}", dir)));
        }

        let mut entries = tokio::fs::read_dir(path).await
            .map_err(|e| SigmaError::Parse(format!("Failed to read directory {}: {}", dir, e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| SigmaError::Parse(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yml") {
                match self.load_rule_file(&path).await {
                    Ok(_) => {},
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

        Ok(())
    }

    /// Load a single rule file
    async fn load_rule_file(&mut self, path: &Path) -> SigmaResult<()> {
        debug!("Loading rule from {}", path.display());
        
        let contents = tokio::fs::read(path).await
            .map_err(|e| SigmaError::Parse(format!("Failed to read file {}: {}", path.display(), e)))?;
        
        let rule = rule_from_yaml(&contents)?;
        self.add_rule(rule).await?;
        
        Ok(())
    }

    /// Add a rule to the ruleset
    pub async fn add_rule(&mut self, rule: Rule) -> SigmaResult<()> {
        // Create a rule handle
        let rule_handle = RuleHandle::new(rule.clone(), std::path::PathBuf::from("ruleset"));
        
        // Build the detection tree
        let tree = build_tree(rule_handle).await
            .map_err(|e: ParseError| SigmaError::Parse(e.to_string()))?;
        
        // Store the compiled rule
        let index = self.rules.len();
        let rule_id = if rule.id.is_empty() {
            format!("rule_{}", index)
        } else {
            rule.id.clone()
        };
        
        self.rule_index.insert(rule_id.clone(), index);
        self.rules.push(CompiledRule {
            rule,
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

        // Evaluate rules in parallel for better performance
        let tasks: Vec<_> = self.rules
            .iter()
            .filter(|r| r.enabled)
            .map(|compiled_rule| {
                let event = event.clone();
                let tree = compiled_rule.tree.clone();
                let rule = compiled_rule.rule.clone();
                
                tokio::spawn(async move {
                    let rule_start = std::time::Instant::now();
                    let ast_event = AstEventAdapter::new(&event);
                    let (matched, applicable) = tree.match_event(&ast_event).await;
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
                }
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
                rule.enabled = enabled;
                if enabled {
                    self.metadata.enabled_rules += 1;
                } else {
                    self.metadata.enabled_rules -= 1;
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
    async fn test_empty_ruleset() {
        let ruleset = RuleSet::new();
        assert!(ruleset.is_empty());
        assert_eq!(ruleset.len(), 0);
        
        let event = DynamicEvent::new(json!({
            "EventID": 1,
            "CommandLine": "test.exe"
        }));
        
        let result = ruleset.evaluate(&event).await.unwrap();
        assert_eq!(result.rules_evaluated, 0);
        assert!(result.matches.is_empty());
    }

    #[tokio::test]
    async fn test_add_rule() {
        let mut ruleset = RuleSet::new();
        
        let rule_yaml = br#"
        title: Test Rule
        id: test-rule-1
        detection:
            selection:
                EventID: 1
            condition: selection
        "#;
        
        let rule = rule_from_yaml(rule_yaml).unwrap();
        ruleset.add_rule(rule).await.unwrap();
        
        assert_eq!(ruleset.len(), 1);
        assert!(!ruleset.is_empty());
    }

    #[tokio::test]
    async fn test_rule_evaluation() {
        let mut ruleset = RuleSet::new();
        
        // Add a matching rule
        let rule_yaml = br#"
        title: Process Creation
        id: proc-create-1
        detection:
            selection:
                EventID: 1
                CommandLine|contains: 'powershell'
            condition: selection
        "#;
        
        let rule = rule_from_yaml(rule_yaml).unwrap();
        ruleset.add_rule(rule).await.unwrap();
        
        // Test matching event
        let event = DynamicEvent::new(json!({
            "EventID": 1,
            "CommandLine": "powershell.exe -Command Get-Process"
        }));
        
        let result = ruleset.evaluate(&event).await.unwrap();
        assert_eq!(result.rules_evaluated, 1);
        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].matched);
        
        // Test non-matching event
        let event = DynamicEvent::new(json!({
            "EventID": 1,
            "CommandLine": "notepad.exe"
        }));
        
        let result = ruleset.evaluate(&event).await.unwrap();
        assert_eq!(result.rules_evaluated, 1);
        assert_eq!(result.matches.len(), 1);
        assert!(!result.matches[0].matched);
    }

    #[tokio::test]
    async fn test_concurrent_ruleset() {
        let ruleset = RuleSet::new();
        let concurrent = ConcurrentRuleSet::new(ruleset);
        
        let rule_yaml = br#"
        title: Test Rule
        id: test-rule-1
        detection:
            selection:
                EventID: 1
            condition: selection
        "#;
        
        let rule = rule_from_yaml(rule_yaml).unwrap();
        concurrent.add_rule(rule).await.unwrap();
        
        assert_eq!(concurrent.len().await, 1);
        
        let event = DynamicEvent::new(json!({
            "EventID": 1
        }));
        
        let result = concurrent.evaluate(&event).await.unwrap();
        assert_eq!(result.rules_evaluated, 1);
    }
}
//! Core Sigma engine implementation

use std::sync::Arc;
use crate::{Result, SigmaEngineBuilder, RuleSet};

/// The main Sigma rule evaluation engine
#[derive(Debug, Clone)]
pub struct SigmaEngine {
    /// The loaded ruleset
    pub ruleset: Arc<RuleSet>,
    /// Engine configuration
    pub config: SigmaEngineBuilder,
}

impl SigmaEngine {
    /// Create a new Sigma engine from a builder configuration
    pub async fn new(builder: SigmaEngineBuilder) -> Result<Self> {
        // Load rules from directories
        let mut ruleset = RuleSet::new();
        
        for dir in &builder.rule_dirs {
            match ruleset.load_directory(dir).await {
                Ok(_) => {},
                Err(e) => {
                    if builder.fail_on_parse_error {
                        return Err(e.into());
                    } else {
                        tracing::warn!("Failed to load rules from {}: {}", dir, e);
                    }
                }
            }
        }
        
        Ok(Self {
            ruleset: Arc::new(ruleset),
            config: builder,
        })
    }
    
    /// Get the loaded ruleset
    pub fn ruleset(&self) -> &RuleSet {
        &self.ruleset
    }
    
    /// Process a single event
    pub async fn process_event(&self, event: crate::DynamicEvent) -> Result<crate::RuleSetResult> {
        self.ruleset.evaluate(&event).await
    }
    
    /// Run the engine (placeholder for actual implementation)
    pub async fn run(self) -> Result<()> {
        // This would be implemented with actual engine logic
        // For now, just a placeholder
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_engine_creation() {
        let builder = SigmaEngineBuilder::new();
        let engine = SigmaEngine::new(builder).await.unwrap();
        assert_eq!(engine.ruleset().len(), 0);
    }
}
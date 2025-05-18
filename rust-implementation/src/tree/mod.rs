use std::sync::Arc;

use crate::ast::Branch;
use crate::core::Event;
use crate::result::Result;
use crate::rule::RuleHandle;

pub mod builder;

pub use builder::build_tree;

/// Tree represents the full AST for a sigma rule
#[derive(Debug)]
pub struct Tree {
    pub root: Arc<dyn Branch>,
    pub rule: Arc<RuleHandle>,
}

impl Tree {
    /// Create a new Tree with the given root branch and rule handle
    pub fn new(root: Arc<dyn Branch>, rule: Arc<RuleHandle>) -> Self {
        Self { root, rule }
    }
    
    /// Match implements the Matcher interface
    pub async fn match_event(&self, event: &dyn crate::ast::Event) -> (bool, bool) {
        let result = self.root.matches(event).await;
        (result.matched, result.applicable)
    }
    
    /// Evaluate an event against this tree, returning a Result if it matches
    pub async fn eval(&self, event: &dyn crate::ast::Event) -> (Option<crate::result::Result>, bool) {
        let (matched, applicable) = self.match_event(event).await;
        
        if !applicable {
            return (None, false);
        }
        
        if matched {
            let result = crate::result::Result::new(
                self.rule.rule.id.clone(),
                self.rule.rule.title.clone(),
                self.rule.rule.description.clone().unwrap_or_default(),
            )
            .with_tags(self.rule.rule.tags.clone());
            
            return (Some(result), true);
        }
        
        (None, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{FieldRule, FieldPattern};
    use crate::ast::nodes::Identifier;
    use crate::event::adapter::SimpleEvent;
    use crate::rule::{Rule, Detection, Logsource};
    use std::path::PathBuf;
    use std::collections::HashMap;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_tree_eval() {
        // Create a simple rule
        let rule = Rule {
            id: "test-123".to_string(),
            title: "Test Rule".to_string(),
            description: Some("Test Description".to_string()),
            author: None,
            level: Some("medium".to_string()),
            status: Some("experimental".to_string()),
            date: None,
            modified: None,
            references: vec![],
            falsepositives: vec![],
            fields: vec![],
            logsource: Logsource::default(),
            detection: Detection::new(),
            tags: vec!["attack.discovery".to_string()],
        };
        
        let rule_handle = Arc::new(RuleHandle::new(rule, PathBuf::from("test.yml")));
        
        // Create a simple field rule that matches EventID=1
        let field_rule = FieldRule::new(
            "EventID".to_string(),
            FieldPattern::Exact("1".to_string()),
        );
        let identifier = Arc::new(Identifier::from_rule(field_rule));
        
        let tree = Tree::new(identifier, rule_handle);
        
        // Test with matching event
        let event = SimpleEvent::new(json!({
            "EventID": "1"
        }));
        
        let (result, applicable) = tree.eval(&event).await;
        assert!(applicable);
        assert!(result.is_some());
        
        let result = result.unwrap();
        assert_eq!(result.id, "test-123");
        assert_eq!(result.title, "Test Rule");
        assert_eq!(result.description, "Test Description");
        assert_eq!(result.tags, vec!["attack.discovery"]);
        
        // Test with non-matching event
        let event = SimpleEvent::new(json!({
            "EventID": "2"
        }));
        
        let (result, applicable) = tree.eval(&event).await;
        assert!(applicable);
        assert!(result.is_none());
    }
}
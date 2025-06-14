use std::sync::Arc;

use crate::ast::Branch;
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
    pub async fn match_event(&self, event: &dyn crate::event::Event) -> (bool, bool) {
        let result = self.root.matches(event).await;
        (result.matched, result.applicable)
    }

    /// Evaluate an event against this tree, returning a Result if it matches
    pub async fn eval(
        &self,
        event: &dyn crate::event::Event,
    ) -> (Option<crate::result::Result>, bool) {
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

        (None, applicable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::nodes::Identifier;
    use crate::ast::{FieldPattern, FieldRule};
    use crate::event::SimpleEvent;
    use crate::rule::{Detection, Logsource, Rule};
    use serde_json::json;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_tree_eval() {
        // Create a simple rule
        let rule = Rule {
            id: "12345678-1234-1234-1234-123456789007".to_string(),
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
            Arc::from("EventID"),
            FieldPattern::String {
                matcher: Arc::new(crate::pattern::string_matcher::ContentPattern {
                    token: Arc::from("1"),
                    lowercase: false,
                    no_collapse_ws: false,
                }),
                pattern_desc: Arc::from("1"),
            },
        );
        let identifier = Arc::new(Identifier::from_rule(field_rule));

        let tree = Tree::new(identifier, rule_handle);

        // Test with matching event
        let mut fields = serde_json::Map::new();
        fields.insert("EventID".to_string(), json!("1"));
        let event = SimpleEvent::new(fields);

        let (result, applicable) = tree.eval(&event).await;
        assert!(applicable);
        assert!(result.is_some());

        if let Some(result) = result {
            assert_eq!(result.id, "12345678-1234-1234-1234-123456789007");
            assert_eq!(result.title, "Test Rule");
            assert_eq!(result.description, "Test Description");
            assert_eq!(result.tags, vec!["attack.discovery"]);
        } else {
            panic!("Expected result to be Some when event matches");
        }

        // Test with non-matching event
        let mut fields2 = serde_json::Map::new();
        fields2.insert("EventID".to_string(), json!("2"));
        let event = SimpleEvent::new(fields2);

        let (result, applicable) = tree.eval(&event).await;
        // When the field exists but value doesn't match, applicable is true but result is None
        assert!(applicable);
        assert!(result.is_none());
    }
}

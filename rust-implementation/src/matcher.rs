/// Core matching traits for the Sigma rule engine
use std::any::Any;
use async_trait::async_trait;
use crate::event::Event;

/// Core matcher trait for evaluating events against patterns
pub trait Matcher: Send + Sync {
    /// Evaluate if an event matches this pattern
    /// Returns (matched, applicable) where applicable indicates if the rule applies to this event type
    fn matches(&self, event: &dyn Event) -> (bool, bool);
}

/// Extended matcher trait that supports tree traversal
pub trait Branch: Matcher {
    /// Get a reference to self as Any for downcasting
    fn as_any(&self) -> &dyn Any;
    
    /// Get a description of this branch for debugging
    fn describe(&self) -> String;
}

/// Async version of the Matcher trait
#[async_trait]
pub trait AsyncMatcher: Send + Sync {
    /// Async evaluation of event matching
    async fn matches_async(&self, event: &dyn Event) -> anyhow::Result<(bool, bool)>;
}

/// Logical AND node that connects multiple branches
pub struct NodeAnd {
    pub left: Box<dyn Branch>,
    pub right: Box<dyn Branch>,
}

impl Matcher for NodeAnd {
    fn matches(&self, event: &dyn Event) -> (bool, bool) {
        let (left_match, left_applicable) = self.left.matches(event);
        if !left_match {
            return (false, left_applicable);
        }
        
        let (right_match, right_applicable) = self.right.matches(event);
        (left_match && right_match, left_applicable && right_applicable)
    }
}

impl Branch for NodeAnd {
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn describe(&self) -> String {
        format!("({} AND {})", self.left.describe(), self.right.describe())
    }
}

/// Logical OR node that connects multiple branches
pub struct NodeOr {
    pub left: Box<dyn Branch>,
    pub right: Box<dyn Branch>,
}

impl Matcher for NodeOr {
    fn matches(&self, event: &dyn Event) -> (bool, bool) {
        let (left_match, left_applicable) = self.left.matches(event);
        if left_match {
            return (true, left_applicable);
        }
        
        let (right_match, right_applicable) = self.right.matches(event);
        (left_match || right_match, left_applicable || right_applicable)
    }
}

impl Branch for NodeOr {
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn describe(&self) -> String {
        format!("({} OR {})", self.left.describe(), self.right.describe())
    }
}

/// Logical NOT node that negates a branch
pub struct NodeNot {
    pub branch: Box<dyn Branch>,
}

impl Matcher for NodeNot {
    fn matches(&self, event: &dyn Event) -> (bool, bool) {
        let (matched, applicable) = self.branch.matches(event);
        if !applicable {
            return (matched, applicable);
        }
        (!matched, applicable)
    }
}

impl Branch for NodeNot {
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn describe(&self) -> String {
        format!("NOT ({})", self.branch.describe())
    }
}

/// List of matchers connected with logical AND
pub struct SimpleAnd {
    pub branches: Vec<Box<dyn Branch>>,
}

impl Matcher for SimpleAnd {
    fn matches(&self, event: &dyn Event) -> (bool, bool) {
        for branch in &self.branches {
            let (matched, applicable) = branch.matches(event);
            if !matched || !applicable {
                return (matched, applicable);
            }
        }
        (true, true)
    }
}

impl Branch for SimpleAnd {
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn describe(&self) -> String {
        let descriptions: Vec<String> = self.branches.iter()
            .map(|b| b.describe())
            .collect();
        format!("({})", descriptions.join(" AND "))
    }
}

impl SimpleAnd {
    /// Reduce the AND node if it only contains one or two elements
    pub fn reduce(self) -> Box<dyn Branch> {
        match self.branches.len() {
            0 => panic!("Empty AND node"),
            1 => self.branches.into_iter().next().unwrap(),
            2 => {
                let mut iter = self.branches.into_iter();
                Box::new(NodeAnd {
                    left: iter.next().unwrap(),
                    right: iter.next().unwrap(),
                })
            }
            _ => Box::new(self),
        }
    }
}

/// List of matchers connected with logical OR
pub struct SimpleOr {
    pub branches: Vec<Box<dyn Branch>>,
}

impl Matcher for SimpleOr {
    fn matches(&self, event: &dyn Event) -> (bool, bool) {
        let mut one_applicable = false;
        
        for branch in &self.branches {
            let (matched, applicable) = branch.matches(event);
            if matched {
                return (true, true);
            }
            if applicable {
                one_applicable = true;
            }
        }
        
        (false, one_applicable)
    }
}

impl Branch for SimpleOr {
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn describe(&self) -> String {
        let descriptions: Vec<String> = self.branches.iter()
            .map(|b| b.describe())
            .collect();
        format!("({})", descriptions.join(" OR "))
    }
}

impl SimpleOr {
    /// Reduce the OR node if it only contains one or two elements
    pub fn reduce(self) -> Box<dyn Branch> {
        match self.branches.len() {
            0 => panic!("Empty OR node"),
            1 => self.branches.into_iter().next().unwrap(),
            2 => {
                let mut iter = self.branches.into_iter();
                Box::new(NodeOr {
                    left: iter.next().unwrap(),
                    right: iter.next().unwrap(),
                })
            }
            _ => Box::new(self),
        }
    }
}

/// Helper function to create a negated branch if needed
pub fn new_node_not_if_negated(branch: Box<dyn Branch>, negated: bool) -> Box<dyn Branch> {
    if negated {
        Box::new(NodeNot { branch })
    } else {
        branch
    }
}

/// Create a conjunction from a list of branches
pub fn new_conjunction(branches: Vec<Box<dyn Branch>>) -> Box<dyn Branch> {
    let and = SimpleAnd { branches };
    and.reduce()
}

/// Create a disjunction from a list of branches
pub fn new_disjunction(branches: Vec<Box<dyn Branch>>) -> Box<dyn Branch> {
    let or = SimpleOr { branches };
    or.reduce()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::DynamicEvent;
    
    struct MockMatcher {
        result: bool,
        applicable: bool,
    }
    
    impl Matcher for MockMatcher {
        fn matches(&self, _event: &dyn Event) -> (bool, bool) {
            (self.result, self.applicable)
        }
    }
    
    impl Branch for MockMatcher {
        fn as_any(&self) -> &dyn Any {
            self
        }
        
        fn describe(&self) -> String {
            format!("Mock({})", self.result)
        }
    }
    
    #[test]
    fn test_node_and() {
        let event = DynamicEvent::new(serde_json::json!({}));
        
        let node = NodeAnd {
            left: Box::new(MockMatcher { result: true, applicable: true }),
            right: Box::new(MockMatcher { result: true, applicable: true }),
        };
        
        let (matched, applicable) = node.matches(&event);
        assert!(matched);
        assert!(applicable);
        
        let node = NodeAnd {
            left: Box::new(MockMatcher { result: true, applicable: true }),
            right: Box::new(MockMatcher { result: false, applicable: true }),
        };
        
        let (matched, applicable) = node.matches(&event);
        assert!(!matched);
        assert!(applicable);
    }
    
    #[test]
    fn test_node_or() {
        let event = DynamicEvent::new(serde_json::json!({}));
        
        let node = NodeOr {
            left: Box::new(MockMatcher { result: false, applicable: true }),
            right: Box::new(MockMatcher { result: true, applicable: true }),
        };
        
        let (matched, applicable) = node.matches(&event);
        assert!(matched);
        assert!(applicable);
        
        let node = NodeOr {
            left: Box::new(MockMatcher { result: false, applicable: true }),
            right: Box::new(MockMatcher { result: false, applicable: true }),
        };
        
        let (matched, applicable) = node.matches(&event);
        assert!(!matched);
        assert!(applicable);
    }
    
    #[test]
    fn test_node_not() {
        let event = DynamicEvent::new(serde_json::json!({}));
        
        let node = NodeNot {
            branch: Box::new(MockMatcher { result: false, applicable: true }),
        };
        
        let (matched, applicable) = node.matches(&event);
        assert!(matched);
        assert!(applicable);
        
        let node = NodeNot {
            branch: Box::new(MockMatcher { result: true, applicable: true }),
        };
        
        let (matched, applicable) = node.matches(&event);
        assert!(!matched);
        assert!(applicable);
    }
}
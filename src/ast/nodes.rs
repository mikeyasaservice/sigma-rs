use super::{Branch, Event, FieldRule, MatchResult};
use async_trait::async_trait;
use std::sync::Arc;

/// Node for logical AND operation
#[derive(Debug, Clone)]
pub struct NodeAnd {
    pub left: Arc<dyn Branch>,
    pub right: Arc<dyn Branch>,
}

impl NodeAnd {
    pub fn new(left: Arc<dyn Branch>, right: Arc<dyn Branch>) -> Self {
        Self { left, right }
    }
}

#[async_trait]
impl Branch for NodeAnd {
    async fn matches(&self, event: &dyn Event) -> MatchResult {
        let left_result = self.left.matches(event).await;
        if !left_result.matched {
            return MatchResult::new(false, left_result.applicable);
        }
        
        let right_result = self.right.matches(event).await;
        MatchResult::new(
            left_result.matched && right_result.matched,
            left_result.applicable && right_result.applicable,
        )
    }

    fn describe(&self) -> String {
        format!("({} AND {})", self.left.describe(), self.right.describe())
    }
}

/// Node for logical OR operation
#[derive(Debug, Clone)]
pub struct NodeOr {
    pub left: Arc<dyn Branch>,
    pub right: Arc<dyn Branch>,
}

impl NodeOr {
    pub fn new(left: Arc<dyn Branch>, right: Arc<dyn Branch>) -> Self {
        Self { left, right }
    }
}

#[async_trait]
impl Branch for NodeOr {
    async fn matches(&self, event: &dyn Event) -> MatchResult {
        let left_result = self.left.matches(event).await;
        if left_result.matched {
            return MatchResult::new(true, left_result.applicable);
        }
        
        let right_result = self.right.matches(event).await;
        MatchResult::new(
            left_result.matched || right_result.matched,
            left_result.applicable || right_result.applicable,
        )
    }

    fn describe(&self) -> String {
        format!("({} OR {})", self.left.describe(), self.right.describe())
    }
}

/// Node for logical NOT operation
#[derive(Debug, Clone)]
pub struct NodeNot {
    pub branch: Arc<dyn Branch>,
}

impl NodeNot {
    pub fn new(branch: Arc<dyn Branch>) -> Self {
        Self { branch }
    }
}

#[async_trait]
impl Branch for NodeNot {
    async fn matches(&self, event: &dyn Event) -> MatchResult {
        let result = self.branch.matches(event).await;
        if !result.applicable {
            return result;
        }
        MatchResult::new(!result.matched, result.applicable)
    }

    fn describe(&self) -> String {
        format!("NOT {})", self.branch.describe())
    }
}

/// Simple AND node for multiple branches
#[derive(Debug, Clone)]
pub struct NodeSimpleAnd {
    pub branches: Vec<Arc<dyn Branch>>,
}

impl NodeSimpleAnd {
    pub fn new(branches: Vec<Arc<dyn Branch>>) -> Self {
        Self { branches }
    }

    /// Reduce to more efficient representation if possible
    pub fn reduce(self) -> Arc<dyn Branch> {
        match self.branches.len() {
            0 => panic!("Cannot reduce empty AND node"),
            1 => self.branches.into_iter().next().unwrap(),
            2 => {
                let mut iter = self.branches.into_iter();
                let left = iter.next().unwrap();
                let right = iter.next().unwrap();
                Arc::new(NodeAnd::new(left, right))
            }
            _ => Arc::new(self),
        }
    }
}

#[async_trait]
impl Branch for NodeSimpleAnd {
    async fn matches(&self, event: &dyn Event) -> MatchResult {
        for branch in &self.branches {
            let result = branch.matches(event).await;
            if !result.matched || !result.applicable {
                return result;
            }
        }
        MatchResult::matched()
    }

    fn describe(&self) -> String {
        let descriptions: Vec<String> = self.branches
            .iter()
            .map(|b| b.describe())
            .collect();
        format!("({})", descriptions.join(" AND "))
    }
}

/// Simple OR node for multiple branches
#[derive(Debug, Clone)]
pub struct NodeSimpleOr {
    pub branches: Vec<Arc<dyn Branch>>,
}

impl NodeSimpleOr {
    pub fn new(branches: Vec<Arc<dyn Branch>>) -> Self {
        Self { branches }
    }

    /// Reduce to more efficient representation if possible
    pub fn reduce(self) -> Arc<dyn Branch> {
        match self.branches.len() {
            0 => panic!("Cannot reduce empty OR node"),
            1 => self.branches.into_iter().next().unwrap(),
            2 => {
                let mut iter = self.branches.into_iter();
                let left = iter.next().unwrap();
                let right = iter.next().unwrap();
                Arc::new(NodeOr::new(left, right))
            }
            _ => Arc::new(self),
        }
    }
}

#[async_trait]
impl Branch for NodeSimpleOr {
    async fn matches(&self, event: &dyn Event) -> MatchResult {
        let mut one_applicable = false;
        
        for branch in &self.branches {
            let result = branch.matches(event).await;
            if result.matched {
                return MatchResult::matched();
            }
            if result.applicable {
                one_applicable = true;
            }
        }
        
        MatchResult::new(false, one_applicable)
    }

    fn describe(&self) -> String {
        let descriptions: Vec<String> = self.branches
            .iter()
            .map(|b| b.describe())
            .collect();
        format!("({})", descriptions.join(" OR "))
    }
}

/// Helper function to create a NOT node if negated
pub fn new_node_not_if_negated(branch: Arc<dyn Branch>, negated: bool) -> Arc<dyn Branch> {
    if negated {
        Arc::new(NodeNot::new(branch))
    } else {
        branch
    }
}

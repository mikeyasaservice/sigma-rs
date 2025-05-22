use super::{Branch, FieldRule, MatchResult};
use crate::event::Event;
use crate::error::SigmaError;
use async_trait::async_trait;
use std::sync::Arc;

/// Node for logical AND operation
#[derive(Debug, Clone)]
pub struct NodeAnd {
    /// Left branch of the AND operation
    pub left: Arc<dyn Branch>,
    /// Right branch of the AND operation
    pub right: Arc<dyn Branch>,
}

impl NodeAnd {
    /// Create a new AND node
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
    /// Left branch of the OR operation
    pub left: Arc<dyn Branch>,
    /// Right branch of the OR operation
    pub right: Arc<dyn Branch>,
}

impl NodeOr {
    /// Create a new OR node
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
    /// Branch to negate
    pub branch: Arc<dyn Branch>,
}

impl NodeNot {
    /// Create a new NOT node
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
    /// Collection of branches that must all match
    pub branches: Vec<Arc<dyn Branch>>,
}

impl NodeSimpleAnd {
    /// Create a new AND node with multiple branches
    pub fn new(branches: Vec<Arc<dyn Branch>>) -> Self {
        Self { branches }
    }

    /// Reduce to more efficient representation if possible
    pub fn reduce(self) -> Result<Arc<dyn Branch>, SigmaError> {
        let mut iter = self.branches.into_iter();
        match (iter.next(), iter.next(), iter.len()) {
            (None, _, _) => Err(SigmaError::InvalidMatcher(
                "Cannot reduce empty AND node - this indicates a parser bug".to_string()
            )),
            (Some(single), None, 0) => Ok(single),
            (Some(left), Some(right), 0) => Ok(Arc::new(NodeAnd::new(left, right))),
            (Some(first), second, _) => {
                // Reconstruct vector for multi-branch case
                let mut branches = vec![first];
                if let Some(second) = second {
                    branches.push(second);
                }
                branches.extend(iter);
                Ok(Arc::new(NodeSimpleAnd::new(branches)))
            }
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
    /// Collection of branches where at least one must match
    pub branches: Vec<Arc<dyn Branch>>,
}

impl NodeSimpleOr {
    /// Create a new OR node with multiple branches
    pub fn new(branches: Vec<Arc<dyn Branch>>) -> Self {
        Self { branches }
    }

    /// Reduce to more efficient representation if possible
    pub fn reduce(self) -> Result<Arc<dyn Branch>, SigmaError> {
        let mut iter = self.branches.into_iter();
        match (iter.next(), iter.next(), iter.len()) {
            (None, _, _) => Err(SigmaError::InvalidMatcher(
                "Cannot reduce empty OR node - this indicates a parser bug".to_string()
            )),
            (Some(single), None, 0) => Ok(single),
            (Some(left), Some(right), 0) => Ok(Arc::new(NodeOr::new(left, right))),
            (Some(first), second, _) => {
                // Reconstruct vector for multi-branch case
                let mut branches = vec![first];
                if let Some(second) = second {
                    branches.push(second);
                }
                branches.extend(iter);
                Ok(Arc::new(NodeSimpleOr::new(branches)))
            }
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

/// Identifier node that wraps a field rule
#[derive(Debug, Clone)]
pub struct Identifier {
    field_rule: FieldRule,
}

impl Identifier {
    /// Create a new identifier node
    pub fn new(field: String, pattern: super::FieldPattern) -> Self {
        Self {
            field_rule: FieldRule::new(Arc::from(field), pattern),
        }
    }
    
    /// Create an identifier node from an existing field rule
    pub fn from_rule(rule: FieldRule) -> Self {
        Self { field_rule: rule }
    }
}

#[async_trait]
impl Branch for Identifier {
    async fn matches(&self, event: &dyn Event) -> MatchResult {
        self.field_rule.matches(event).await
    }
    
    fn describe(&self) -> String {
        self.field_rule.describe()
    }
}

/// Comparison operators for aggregation conditions
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    /// Greater than comparison (>)
    GreaterThan,
    /// Greater than or equal comparison (>=)
    GreaterOrEqual,
    /// Less than comparison (<)
    LessThan,
    /// Less than or equal comparison (<=)
    LessOrEqual,
    /// Equal comparison (==)
    Equal,
    /// Not equal comparison (!=)
    NotEqual,
}

impl ComparisonOp {
    /// Evaluate the comparison operation
    pub fn evaluate(&self, value: f64, threshold: f64) -> bool {
        match self {
            ComparisonOp::GreaterThan => value > threshold,
            ComparisonOp::GreaterOrEqual => value >= threshold,
            ComparisonOp::LessThan => value < threshold,
            ComparisonOp::LessOrEqual => value <= threshold,
            ComparisonOp::Equal => (value - threshold).abs() < f64::EPSILON,
            ComparisonOp::NotEqual => (value - threshold).abs() >= f64::EPSILON,
        }
    }
}

/// Node for aggregation operations
#[derive(Debug, Clone)]
pub struct NodeAggregation {
    /// Aggregation function to apply
    pub function: crate::aggregation::AggregationFunction,
    /// Comparison operator for the threshold
    pub comparison: ComparisonOp,
    /// Threshold value for comparison
    pub threshold: f64,
    /// Field to group by (optional)
    pub by_field: Option<String>,
    /// Time window for aggregation (optional)
    pub time_window: Option<std::time::Duration>,
}

impl NodeAggregation {
    /// Create a new aggregation node
    pub fn new(
        function: crate::aggregation::AggregationFunction,
        comparison: ComparisonOp,
        threshold: f64,
        by_field: Option<String>,
        time_window: Option<std::time::Duration>,
    ) -> Self {
        Self {
            function,
            comparison,
            threshold,
            by_field,
            time_window,
        }
    }
}

#[async_trait]
impl Branch for NodeAggregation {
    async fn matches(&self, _event: &dyn Event) -> MatchResult {
        // Aggregation logic will be implemented by the AggregationEvaluator
        // This is just a placeholder for the AST node
        MatchResult::not_matched()
    }

    fn describe(&self) -> String {
        format!(
            "AGGREGATE({:?} {} {} BY {:?} WITHIN {:?})",
            self.function,
            match self.comparison {
                ComparisonOp::GreaterThan => ">",
                ComparisonOp::GreaterOrEqual => ">=",
                ComparisonOp::LessThan => "<",
                ComparisonOp::LessOrEqual => "<=",
                ComparisonOp::Equal => "==",
                ComparisonOp::NotEqual => "!=",
            },
            self.threshold,
            self.by_field,
            self.time_window
        )
    }
}

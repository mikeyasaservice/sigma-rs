use chrono::{DateTime, Utc};

pub mod sliding_window;
pub mod evaluator;
pub mod config;

pub use sliding_window::SlidingWindow;
pub use evaluator::AggregationEvaluator;
pub use config::{AggregationConfig, WindowConfig};

/// Aggregation functions supported by Sigma
#[derive(Debug, Clone, PartialEq)]
pub enum AggregationFunction {
    Count,
    Sum(String), // field name
    Average(String), // field name
    Min(String), // field name
    Max(String), // field name
}

/// Result of an aggregation evaluation
#[derive(Debug, Clone)]
pub struct AggregationResult {
    /// Whether the aggregation condition was triggered
    pub triggered: bool,
    /// The current aggregated value
    pub value: f64,
    /// The group identifier (if grouping was used)
    pub group: Option<String>,
    /// Timestamp of evaluation
    pub timestamp: DateTime<Utc>,
}

/// Statistics about the aggregation evaluator
#[derive(Debug, Clone)]
pub struct AggregationStatistics {
    /// Number of active aggregation groups
    pub active_groups: usize,
    /// Estimated memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Total evaluations performed
    pub total_evaluations: u64,
    /// Cache hit rate
    pub cache_hit_rate: f64,
}
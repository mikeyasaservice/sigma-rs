use chrono::{DateTime, Utc};

/// Sliding window implementation for time-based aggregation
pub mod sliding_window;
/// Aggregation evaluation engine
pub mod evaluator;
/// Configuration types for aggregation
pub mod config;

pub use sliding_window::SlidingWindow;
pub use evaluator::AggregationEvaluator;
pub use config::{AggregationConfig, WindowConfig};

/// Aggregation functions supported by Sigma
#[derive(Debug, Clone, PartialEq)]
pub enum AggregationFunction {
    /// Count aggregation
    Count,
    /// Sum aggregation over a field
    Sum(String),
    /// Average aggregation over a field
    Average(String),
    /// Minimum aggregation over a field
    Min(String),
    /// Maximum aggregation over a field
    Max(String),
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
use std::time::Duration;

/// Configuration for aggregation evaluator
#[derive(Debug, Clone)]
pub struct AggregationConfig {
    /// TTL for aggregation groups
    pub group_ttl: Duration,
    /// How often to clean up expired groups
    pub cleanup_interval: Duration,
    /// Maximum number of groups to cache (to prevent memory exhaustion)
    pub max_cache_size: u64,
    /// Maximum memory usage in bytes for the cache
    pub max_cache_memory: Option<u64>,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            group_ttl: Duration::from_secs(3600),      // 1 hour
            cleanup_interval: Duration::from_secs(60), // 1 minute
            max_cache_size: 10_000,                    // Limit to 10K groups
            max_cache_memory: Some(100 * 1024 * 1024), // 100MB limit
        }
    }
}

/// Configuration for a sliding window
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Duration of the sliding window
    pub duration: Duration,
    /// Field to group by (if any)
    pub by_field: String,
}

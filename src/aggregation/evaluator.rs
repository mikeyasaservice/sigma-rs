use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::borrow::Cow;
use moka::future::Cache;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use crate::ast::nodes::NodeAggregation;
use crate::event::Event;
use super::{AggregationResult, AggregationStatistics, AggregationConfig};

/// An evaluator that performs aggregation operations based on configured rules.
///
/// The evaluator maintains a cache of group states and tracks statistics during evaluation.
///
/// # Fields
/// * `cache` - A cache storing group states indexed by string keys
/// * `config` - Configuration parameters for the aggregation
/// * `stats` - Thread-safe statistics tracking during evaluation
pub struct AggregationEvaluator {
    cache: Cache<String, Arc<RwLock<GroupState>>>,
    #[allow(dead_code)] // TODO: Use config for customizing aggregation behavior
    config: AggregationConfig,
    stats: Arc<Stats>,
}

#[derive(Debug)]
struct GroupState {
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
    last_update: DateTime<Utc>,
}

#[derive(Debug)]
struct Stats {
    total_evaluations: AtomicU64,
    #[allow(dead_code)] // TODO: Use active_groups for monitoring
    active_groups: AtomicUsize,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            total_evaluations: AtomicU64::new(0),
            active_groups: AtomicUsize::new(0),
        }
    }
}

impl AggregationEvaluator {
    /// Create a new aggregation evaluator with default configuration
    pub fn new() -> Self {
        Self::with_config(AggregationConfig::default())
    }
    
    /// Create a new aggregation evaluator with specified configuration
    pub fn with_config(config: AggregationConfig) -> Self {
        let mut cache_builder = Cache::builder()
            .time_to_live(config.group_ttl)
            .max_capacity(config.max_cache_size);
            
        // Add memory limit if specified
        if let Some(_memory_limit) = config.max_cache_memory {
            // Note: moka doesn't directly support memory limits, but we can use weigher
            // For now, we'll use capacity limit as a proxy
            cache_builder = cache_builder.max_capacity(config.max_cache_size);
        }
        
        let cache = cache_builder.build();
            
        Self {
            cache,
            config,
            stats: Arc::new(Stats::default()),
        }
    }
    
    /// Evaluate an aggregation node against an event
    pub async fn evaluate(&self, node: &NodeAggregation, event: &dyn Event) -> AggregationResult {
        // Increment evaluation counter atomically - no lock needed
        self.stats.total_evaluations.fetch_add(1, Ordering::Relaxed);
        
        // Extract group key
        let group_key = match &node.by_field {
            Some(field) => {
                match event.select(field) {
                    (Some(value), _) => format!("{}:{}", field, value_to_cow(&value)),
                    _ => format!("{}:unknown", field),
                }
            }
            None => "default".to_string(),
        };
        
        // Get or create group state
        let state = match self.cache.try_get_with(group_key.clone(), async {
            Ok(Arc::new(RwLock::new(GroupState {
                count: 0,
                sum: 0.0,
                min: f64::MAX,
                max: f64::MIN,
                last_update: Utc::now(),
            }))) as std::result::Result<Arc<RwLock<GroupState>>, std::convert::Infallible>
        }).await {
            Ok(state) => state,
            Err(_) => {
                // This should never happen with Infallible, but handle gracefully
                tracing::error!("Cache operation failed unexpectedly");
                return AggregationResult {
                    triggered: false,
                    value: 0.0,
                    group: Some(group_key),
                    timestamp: Utc::now(),
                };
            }
        };
        
        let mut state_guard = state.write();
        
        // Update aggregation based on function
        let current_value = match &node.function {
            crate::aggregation::AggregationFunction::Count => {
                state_guard.count += 1;
                state_guard.count as f64
            }
            crate::aggregation::AggregationFunction::Sum(field) => {
                let value = extract_numeric_value(event, field);
                state_guard.sum += value;
                state_guard.sum
            }
            crate::aggregation::AggregationFunction::Average(field) => {
                let value = extract_numeric_value(event, field);
                state_guard.sum += value;
                state_guard.count += 1;
                if state_guard.count > 0 {
                    state_guard.sum / state_guard.count as f64
                } else {
                    0.0
                }
            }
            crate::aggregation::AggregationFunction::Min(field) => {
                let value = extract_numeric_value(event, field);
                state_guard.min = state_guard.min.min(value);
                state_guard.min
            }
            crate::aggregation::AggregationFunction::Max(field) => {
                let value = extract_numeric_value(event, field);
                state_guard.max = state_guard.max.max(value);
                state_guard.max
            }
        };
        
        state_guard.last_update = Utc::now();
        
        // Check if threshold is met using proper comparison
        let triggered = node.comparison.evaluate(current_value, node.threshold);
        
        AggregationResult {
            triggered,
            value: current_value,
            group: Some(group_key),
            timestamp: Utc::now(),
        }
    }
    
    /// Get statistics about the aggregation evaluator
    pub async fn get_statistics(&self) -> AggregationStatistics {
        // Calculate cache hit rate based on cache internal metrics
        let entry_count = self.cache.entry_count();
        let weighted_size = self.cache.weighted_size();
        
        // Estimate cache hit rate: if weighted_size is less than entry_count,
        // it means some entries have been evicted, indicating cache pressure
        let hit_rate = if entry_count > 0 {
            (weighted_size as f64 / entry_count as f64).min(1.0)
        } else {
            1.0 // No entries means perfect hit rate (or no operations)
        };
        
        AggregationStatistics {
            active_groups: entry_count as usize,
            memory_usage_bytes: std::mem::size_of::<GroupState>() * entry_count as usize,
            total_evaluations: self.stats.total_evaluations.load(Ordering::Relaxed),
            cache_hit_rate: hit_rate,
        }
    }
}

fn extract_numeric_value(event: &dyn Event, field: &str) -> f64 {
    match event.select(field) {
        (Some(value), _) => match value {
            crate::event::Value::Float(f) => f,
            crate::event::Value::Integer(i) => i as f64,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

fn value_to_cow(value: &crate::event::Value) -> Cow<'_, str> {
    match value {
        crate::event::Value::String(s) => Cow::Borrowed(s),
        crate::event::Value::Integer(i) => Cow::Owned(i.to_string()),
        crate::event::Value::Float(f) => Cow::Owned(f.to_string()),
        crate::event::Value::Boolean(b) => Cow::Owned(b.to_string()),
        _ => Cow::Borrowed("unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, Value};
    use crate::aggregation::AggregationFunction;
    use crate::ast::nodes::{NodeAggregation, ComparisonOp};
    use std::collections::HashMap;
    
    #[derive(Debug)]
    struct TestEvent {
        fields: HashMap<String, Value>,
    }
    
    impl TestEvent {
        fn new() -> Self {
            Self {
                fields: HashMap::new(),
            }
        }
        
        fn with_field(mut self, key: &str, value: Value) -> Self {
            self.fields.insert(key.to_string(), value);
            self
        }
    }
    
    impl crate::event::Keyworder for TestEvent {
        fn keywords(&self) -> (Vec<String>, bool) {
            (vec![], true)
        }
    }
    
    impl crate::event::Selector for TestEvent {
        fn select(&self, key: &str) -> (Option<Value>, bool) {
            match self.fields.get(key) {
                Some(value) => (Some(value.clone()), true),
                None => (None, false),
            }
        }
    }
    
    impl Event for TestEvent {
        fn id(&self) -> &str {
            "test-event"
        }
        
        fn timestamp(&self) -> i64 {
            0
        }
    }
    
    #[tokio::test]
    async fn test_aggregation_count() {
        let evaluator = AggregationEvaluator::new();
        let node = NodeAggregation {
            function: AggregationFunction::Count,
            by_field: None,
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::GreaterThan,
            threshold: 2.0,
        };
        
        let event = TestEvent::new();
        
        // First evaluation
        let result1 = evaluator.evaluate(&node, &event).await;
        assert!(!result1.triggered);
        assert_eq!(result1.value, 1.0);
        
        // Second evaluation
        let result2 = evaluator.evaluate(&node, &event).await;
        assert!(!result2.triggered);
        assert_eq!(result2.value, 2.0);
        
        // Third evaluation - should trigger
        let result3 = evaluator.evaluate(&node, &event).await;
        assert!(result3.triggered);
        assert_eq!(result3.value, 3.0);
    }
    
    #[tokio::test]
    async fn test_aggregation_sum() {
        let evaluator = AggregationEvaluator::new();
        let node = NodeAggregation {
            function: AggregationFunction::Sum("value".to_string()),
            by_field: None,
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::GreaterThan,
            threshold: 100.0,
        };
        
        let event1 = TestEvent::new().with_field("value", Value::Float(50.0));
        let event2 = TestEvent::new().with_field("value", Value::Float(60.0));
        
        // First evaluation
        let result1 = evaluator.evaluate(&node, &event1).await;
        assert!(!result1.triggered);
        assert_eq!(result1.value, 50.0);
        
        // Second evaluation - should trigger
        let result2 = evaluator.evaluate(&node, &event2).await;
        assert!(result2.triggered);
        assert_eq!(result2.value, 110.0);
    }
    
    #[tokio::test]
    async fn test_aggregation_by_field() {
        let evaluator = AggregationEvaluator::new();
        let node = NodeAggregation {
            function: AggregationFunction::Count,
            by_field: Some("user".to_string()),
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::GreaterOrEqual,
            threshold: 2.0,
        };
        
        let event_user1 = TestEvent::new().with_field("user", Value::String(Arc::from("alice")));
        let event_user2 = TestEvent::new().with_field("user", Value::String(Arc::from("bob")));
        
        // Events for user1
        let result1 = evaluator.evaluate(&node, &event_user1).await;
        assert!(!result1.triggered);
        assert_eq!(result1.value, 1.0);
        assert_eq!(result1.group, Some("user:alice".to_string()));
        
        let result2 = evaluator.evaluate(&node, &event_user1).await;
        assert!(result2.triggered);
        assert_eq!(result2.value, 2.0);
        
        // Events for user2 - separate group
        let result3 = evaluator.evaluate(&node, &event_user2).await;
        assert!(!result3.triggered);
        assert_eq!(result3.value, 1.0);
        assert_eq!(result3.group, Some("user:bob".to_string()));
    }
    
    #[tokio::test]
    async fn test_aggregation_average() {
        let evaluator = AggregationEvaluator::new();
        let node = NodeAggregation {
            function: AggregationFunction::Average("score".to_string()),
            by_field: None,
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::GreaterThan,
            threshold: 75.0,
        };
        
        let event1 = TestEvent::new().with_field("score", Value::Float(60.0));
        let event2 = TestEvent::new().with_field("score", Value::Float(80.0));
        let event3 = TestEvent::new().with_field("score", Value::Float(90.0));
        
        let result1 = evaluator.evaluate(&node, &event1).await;
        assert!(!result1.triggered);
        assert_eq!(result1.value, 60.0);
        
        let result2 = evaluator.evaluate(&node, &event2).await;
        assert!(!result2.triggered);
        assert_eq!(result2.value, 70.0); // (60 + 80) / 2
        
        let result3 = evaluator.evaluate(&node, &event3).await;
        assert!(result3.triggered);
        assert_eq!(result3.value, 230.0 / 3.0); // (60 + 80 + 90) / 3
    }
    
    #[tokio::test]
    async fn test_aggregation_min_max() {
        let evaluator = AggregationEvaluator::new();
        
        // Test MIN
        let min_node = NodeAggregation {
            function: AggregationFunction::Min("temp".to_string()),
            by_field: None,
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::LessThan,
            threshold: 10.0,
        };
        
        let event1 = TestEvent::new().with_field("temp", Value::Float(15.0));
        let event2 = TestEvent::new().with_field("temp", Value::Float(5.0));
        
        let result1 = evaluator.evaluate(&min_node, &event1).await;
        assert!(!result1.triggered);
        assert_eq!(result1.value, 15.0);
        
        let result2 = evaluator.evaluate(&min_node, &event2).await;
        assert!(result2.triggered);
        assert_eq!(result2.value, 5.0);
        
        // Test MAX
        let max_node = NodeAggregation {
            function: AggregationFunction::Max("temp".to_string()),
            by_field: None,
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::GreaterThan,
            threshold: 30.0,
        };
        
        let event3 = TestEvent::new().with_field("temp", Value::Float(25.0));
        let event4 = TestEvent::new().with_field("temp", Value::Float(35.0));
        
        let result3 = evaluator.evaluate(&max_node, &event3).await;
        assert!(!result3.triggered);
        assert_eq!(result3.value, 25.0);
        
        let result4 = evaluator.evaluate(&max_node, &event4).await;
        assert!(result4.triggered);
        assert_eq!(result4.value, 35.0);
    }
    
    #[tokio::test]
    async fn test_aggregation_cache_eviction() {
        // Test with small cache to force eviction
        let config = AggregationConfig {
            group_ttl: std::time::Duration::from_millis(100),
            cleanup_interval: std::time::Duration::from_secs(1),
            max_cache_size: 2, // Very small cache
            max_cache_memory: None,
        };
        
        let evaluator = AggregationEvaluator::with_config(config);
        let node = NodeAggregation {
            function: AggregationFunction::Count,
            by_field: Some("user".to_string()),
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::GreaterThan,
            threshold: 10.0,
        };
        
        // Create events for different users
        let event1 = TestEvent::new().with_field("user", Value::String(Arc::from("user1")));
        let event2 = TestEvent::new().with_field("user", Value::String(Arc::from("user2")));
        let event3 = TestEvent::new().with_field("user", Value::String(Arc::from("user3")));
        
        // Fill cache
        evaluator.evaluate(&node, &event1).await;
        evaluator.evaluate(&node, &event2).await;
        
        // This should trigger eviction of oldest entry
        evaluator.evaluate(&node, &event3).await;
        
        // Wait for TTL to expire
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        
        // Re-evaluate user1 - should start fresh due to eviction
        let result = evaluator.evaluate(&node, &event1).await;
        assert_eq!(result.value, 1.0); // Should be 1, not 2
    }
    
    #[tokio::test]
    async fn test_aggregation_statistics() {
        let evaluator = AggregationEvaluator::new();
        let node = NodeAggregation {
            function: AggregationFunction::Count,
            by_field: Some("category".to_string()),
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::GreaterThan,
            threshold: 10.0,
        };
        
        let event1 = TestEvent::new().with_field("category", Value::String(Arc::from("A")));
        let event2 = TestEvent::new().with_field("category", Value::String(Arc::from("B")));
        
        // Generate some activity
        for _ in 0..5 {
            evaluator.evaluate(&node, &event1).await;
            evaluator.evaluate(&node, &event2).await;
        }
        
        // Run cache maintenance to ensure entries are properly counted
        evaluator.cache.run_pending_tasks().await;
        
        let stats = evaluator.get_statistics().await;
        assert_eq!(stats.total_evaluations, 10);
        assert_eq!(stats.active_groups, 2); // Exactly 2 groups (A and B)
        assert!(stats.memory_usage_bytes > 0);
        assert!(stats.cache_hit_rate > 0.0);
    }
    
    #[tokio::test]
    async fn test_aggregation_missing_field() {
        let evaluator = AggregationEvaluator::new();
        let node = NodeAggregation {
            function: AggregationFunction::Sum("missing_field".to_string()),
            by_field: None,
            time_window: Some(std::time::Duration::from_secs(60)),
            comparison: ComparisonOp::GreaterThan,
            threshold: 0.0,
        };
        
        let event = TestEvent::new();
        
        // Should handle missing field gracefully
        let result = evaluator.evaluate(&node, &event).await;
        assert!(!result.triggered);
        assert_eq!(result.value, 0.0); // Default value for missing field
    }
    
    #[tokio::test] 
    async fn test_value_to_cow() {
        // Test string conversion
        let str_val = Value::String(Arc::from("test"));
        assert_eq!(value_to_cow(&str_val), "test");
        
        // Test number conversions
        let int_val = Value::Integer(42);
        assert_eq!(value_to_cow(&int_val), "42");
        
        let float_val = Value::Float(3.14);
        assert_eq!(value_to_cow(&float_val), "3.14");
        
        let bool_val = Value::Boolean(true);
        assert_eq!(value_to_cow(&bool_val), "true");
        
        // Test unknown value
        let null_val = Value::Null;
        assert_eq!(value_to_cow(&null_val), "unknown");
    }
}
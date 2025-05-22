use std::sync::Arc;
use tokio::sync::RwLock;
use moka::future::Cache;
use chrono::{DateTime, Utc};
use crate::ast::nodes::NodeAggregation;
use crate::event::Event;
use super::{AggregationResult, AggregationStatistics, AggregationConfig};

/// Mock aggregation evaluator for testing - will be fully implemented later
pub struct AggregationEvaluator {
    cache: Cache<String, Arc<RwLock<GroupState>>>,
    config: AggregationConfig,
    stats: Arc<RwLock<Stats>>,
}

#[derive(Debug)]
struct GroupState {
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
    last_update: DateTime<Utc>,
}

#[derive(Debug, Default)]
struct Stats {
    total_evaluations: u64,
    active_groups: usize,
}

impl AggregationEvaluator {
    pub fn new() -> Self {
        Self::with_config(AggregationConfig::default())
    }
    
    pub fn with_config(config: AggregationConfig) -> Self {
        let cache = Cache::builder()
            .time_to_live(config.group_ttl)
            .build();
            
        Self {
            cache,
            config,
            stats: Arc::new(RwLock::new(Stats::default())),
        }
    }
    
    pub async fn evaluate(&self, node: &NodeAggregation, event: &dyn Event) -> AggregationResult {
        let mut stats = self.stats.write().await;
        stats.total_evaluations += 1;
        drop(stats);
        
        // Extract group key
        let group_key = match &node.by_field {
            Some(field) => {
                match event.select(field) {
                    (Some(value), _) => format!("{}:{}", field, value_to_string(&value)),
                    _ => format!("{}:unknown", field),
                }
            }
            None => "default".to_string(),
        };
        
        // Get or create group state
        let state = self.cache.try_get_with(group_key.clone(), async {
            Ok(Arc::new(RwLock::new(GroupState {
                count: 0,
                sum: 0.0,
                min: f64::MAX,
                max: f64::MIN,
                last_update: Utc::now(),
            }))) as std::result::Result<Arc<RwLock<GroupState>>, std::convert::Infallible>
        }).await.unwrap();
        
        let mut state_guard = state.write().await;
        
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
        
        // Check if threshold is met
        let triggered = node.comparison.evaluate(current_value, node.threshold);
        
        AggregationResult {
            triggered,
            value: current_value,
            group: Some(group_key),
            timestamp: Utc::now(),
        }
    }
    
    pub async fn get_statistics(&self) -> AggregationStatistics {
        let stats = self.stats.read().await;
        AggregationStatistics {
            active_groups: self.cache.weighted_size() as usize,
            memory_usage_bytes: std::mem::size_of::<GroupState>() * self.cache.weighted_size() as usize,
            total_evaluations: stats.total_evaluations,
            cache_hit_rate: 0.0, // Simplified for testing
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

fn value_to_string(value: &crate::event::Value) -> String {
    match value {
        crate::event::Value::String(s) => s.clone(),
        crate::event::Value::Integer(i) => i.to_string(),
        crate::event::Value::Float(f) => f.to_string(),
        crate::event::Value::Boolean(b) => b.to_string(),
        _ => "unknown".to_string(),
    }
}
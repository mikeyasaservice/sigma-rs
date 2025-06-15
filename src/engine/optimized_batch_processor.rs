//! Optimized batch processor for high-throughput event processing
//!
//! This processor implements the three-stage processing strategy:
//! 1. Extract frequently accessed fields into columnar format
//! 2. Run pattern matchers on columns in parallel
//! 3. Evaluate rule logic using pattern match results

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Semaphore, RwLock};
use arrow::array::{ArrayRef, RecordBatch};
use tracing::{info, trace, instrument};

use crate::error::Result;
use crate::ast::tiered_compiler::{TieredCompiler, TieredRule, BooleanExpression};
use crate::pattern::grouped_matcher::GroupedPatternMatcher;

/// Optimal batch size for cache efficiency
pub const OPTIMAL_BATCH_SIZE: usize = 256 * 1024; // 256K events

/// Match results for a batch
#[derive(Debug)]
pub struct BatchMatchResults {
    /// Rule ID -> Event indices that matched
    pub rule_matches: HashMap<String, Vec<usize>>,
    
    /// Processing statistics
    pub stats: ProcessingStats,
}

/// Processing statistics
#[derive(Debug, Default)]
pub struct ProcessingStats {
    pub events_processed: usize,
    pub rules_evaluated: usize,
    pub matches_found: usize,
    pub processing_time_ms: u64,
    pub pattern_matching_time_ms: u64,
    pub rule_evaluation_time_ms: u64,
}

/// Configuration for the batch processor
#[derive(Debug, Clone)]
pub struct BatchProcessorConfig {
    /// Maximum batch size
    pub batch_size: usize,
    
    /// Number of parallel pattern matching threads
    pub pattern_match_threads: usize,
    
    /// Enable columnar field extraction optimization
    pub enable_field_extraction: bool,
    
    /// Fields to extract for fast access
    pub hot_fields: Vec<String>,
    
    /// Maximum memory per batch (bytes)
    pub max_batch_memory: usize,
    
    /// Enable performance profiling
    pub enable_profiling: bool,
}

impl Default for BatchProcessorConfig {
    fn default() -> Self {
        Self {
            batch_size: OPTIMAL_BATCH_SIZE,
            pattern_match_threads: num_cpus::get(),
            enable_field_extraction: true,
            hot_fields: vec![
                "CommandLine".to_string(),
                "Image".to_string(),
                "ParentImage".to_string(),
                "User".to_string(),
                "EventID".to_string(),
            ],
            max_batch_memory: 1024 * 1024 * 1024, // 1GB
            enable_profiling: cfg!(debug_assertions),
        }
    }
}

/// Optimized batch processor
pub struct OptimizedBatchProcessor {
    /// Configuration
    config: BatchProcessorConfig,
    
    /// Compiled rules from tiered compiler
    tiered_compiler: Arc<TieredCompiler>,
    
    /// Grouped pattern matcher
    pattern_matcher: Arc<GroupedPatternMatcher>,
    
    /// Semaphore for controlling parallelism
    pattern_match_semaphore: Arc<Semaphore>,
    
    /// Field extraction cache
    field_cache: Arc<RwLock<HashMap<String, ArrayRef>>>,
}

impl OptimizedBatchProcessor {
    /// Create a new optimized batch processor
    pub fn new(
        config: BatchProcessorConfig,
        tiered_compiler: Arc<TieredCompiler>,
    ) -> Self {
        let pattern_matcher = tiered_compiler.get_pattern_matcher();
        
        Self {
            pattern_match_semaphore: Arc::new(Semaphore::new(config.pattern_match_threads)),
            config,
            tiered_compiler,
            pattern_matcher,
            field_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Process a batch of events
    #[instrument(skip(self, events))]
    pub async fn process_batch(&self, events: RecordBatch) -> Result<BatchMatchResults> {
        let start_time = std::time::Instant::now();
        let mut stats = ProcessingStats::default();
        stats.events_processed = events.num_rows();
        
        // Stage 1: Extract frequently accessed fields
        let field_extraction_start = std::time::Instant::now();
        let field_columns = if self.config.enable_field_extraction {
            self.extract_hot_fields(&events).await?
        } else {
            HashMap::new()
        };
        trace!("Field extraction took {:?}", field_extraction_start.elapsed());
        
        // Stage 2: Run pattern matchers on columns
        let pattern_match_start = std::time::Instant::now();
        let pattern_matches = self.run_pattern_matchers(&field_columns).await?;
        stats.pattern_matching_time_ms = pattern_match_start.elapsed().as_millis() as u64;
        
        // Stage 3: Evaluate rule logic using pattern results
        let rule_eval_start = std::time::Instant::now();
        let rule_matches = self.evaluate_rules(&pattern_matches, &events).await?;
        stats.rule_evaluation_time_ms = rule_eval_start.elapsed().as_millis() as u64;
        
        // Collect statistics
        stats.rules_evaluated = self.tiered_compiler.get_pattern_rules().len() +
                                self.tiered_compiler.get_simple_rules().len();
        stats.matches_found = rule_matches.values().map(|v| v.len()).sum();
        stats.processing_time_ms = start_time.elapsed().as_millis() as u64;
        
        if self.config.enable_profiling {
            info!(
                "Processed {} events in {}ms (pattern: {}ms, eval: {}ms), {} matches",
                stats.events_processed,
                stats.processing_time_ms,
                stats.pattern_matching_time_ms,
                stats.rule_evaluation_time_ms,
                stats.matches_found
            );
        }
        
        Ok(BatchMatchResults {
            rule_matches,
            stats,
        })
    }
    
    /// Extract hot fields into columnar format for fast access
    async fn extract_hot_fields(
        &self,
        batch: &RecordBatch,
    ) -> Result<HashMap<String, ArrayRef>> {
        let mut field_columns = HashMap::new();
        
        for field_name in &self.config.hot_fields {
            if let Ok(field_idx) = batch.schema().index_of(field_name) {
                let column = batch.column(field_idx).clone();
                field_columns.insert(field_name.clone(), column);
            }
        }
        
        // Cache the extracted fields
        let mut cache = self.field_cache.write().await;
        cache.clear();
        cache.extend(field_columns.clone());
        
        Ok(field_columns)
    }
    
    /// Run pattern matchers on extracted columns in parallel
    async fn run_pattern_matchers(
        &self,
        field_columns: &HashMap<String, ArrayRef>,
    ) -> Result<HashMap<String, HashMap<String, Vec<usize>>>> {
        let mut all_matches = HashMap::new();
        let mut match_tasks = Vec::new();
        
        // Spawn parallel pattern matching tasks for each field
        for (field_name, column) in field_columns {
            let field_name_clone = field_name.clone();
            let column = column.clone();
            let pattern_matcher = Arc::clone(&self.pattern_matcher);
            let semaphore = Arc::clone(&self.pattern_match_semaphore);
            
            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                
                if let Some(field_group) = pattern_matcher.get_field_group(&field_name_clone) {
                    field_group.match_array(&column)
                } else {
                    Ok(HashMap::new())
                }
            });
            
            match_tasks.push((field_name.clone(), task));
        }
        
        // Collect results from all tasks
        for (field_name, task) in match_tasks {
            let matches = task.await
                .map_err(|e| crate::error::SigmaError::Runtime(
                    format!("Pattern match task failed: {}", e)
                ))?;
            all_matches.insert(field_name, matches?);
        }
        
        Ok(all_matches)
    }
    
    /// Evaluate rule logic using pattern match results
    async fn evaluate_rules(
        &self,
        pattern_matches: &HashMap<String, HashMap<String, Vec<usize>>>,
        batch: &RecordBatch,
    ) -> Result<HashMap<String, Vec<usize>>> {
        let mut rule_matches = HashMap::new();
        let batch_size = batch.num_rows();
        
        // Evaluate simple rules first (fastest)
        for rule in self.tiered_compiler.get_simple_rules() {
            if let TieredRule::Simple { rule_id: _, field: _, value: _ } = rule {
                // TODO: Implement simple field equality check
                // For now, skip
                continue;
            }
        }
        
        // Evaluate pattern rules (majority)
        for rule in self.tiered_compiler.get_pattern_rules() {
            if let TieredRule::Pattern { rule_id, required_fields: _, pattern_refs, logic } = rule {
                let mut event_matches = vec![false; batch_size];
                
                // Evaluate boolean expression for each event
                for event_idx in 0..batch_size {
                    if self.evaluate_boolean_expr(
                        &logic,
                        event_idx,
                        pattern_refs,
                        pattern_matches,
                    ) {
                        event_matches[event_idx] = true;
                    }
                }
                
                // Collect matching event indices
                let matching_indices: Vec<usize> = event_matches
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, matched)| if *matched { Some(idx) } else { None })
                    .collect();
                
                if !matching_indices.is_empty() {
                    rule_matches.insert(rule_id.clone(), matching_indices);
                }
            }
        }
        
        // Evaluate complex rules using DataFusion (slowest)
        // TODO: Implement DataFusion evaluation for complex rules
        
        Ok(rule_matches)
    }
    
    /// Evaluate boolean expression for a specific event
    fn evaluate_boolean_expr(
        &self,
        expr: &BooleanExpression,
        event_idx: usize,
        pattern_refs: &[crate::ast::tiered_compiler::PatternRef],
        pattern_matches: &HashMap<String, HashMap<String, Vec<usize>>>,
    ) -> bool {
        match expr {
            BooleanExpression::And(exprs) => {
                exprs.iter().all(|e| self.evaluate_boolean_expr(e, event_idx, pattern_refs, pattern_matches))
            }
            BooleanExpression::Or(exprs) => {
                exprs.iter().any(|e| self.evaluate_boolean_expr(e, event_idx, pattern_refs, pattern_matches))
            }
            BooleanExpression::Not(expr) => {
                !self.evaluate_boolean_expr(expr, event_idx, pattern_refs, pattern_matches)
            }
            BooleanExpression::PatternRef(idx) => {
                if let Some(pattern_ref) = pattern_refs.get(*idx) {
                    if let Some(field_matches) = pattern_matches.get(&pattern_ref.field) {
                        // Check if any rule matched this event
                        let matched = field_matches.values()
                            .any(|indices| indices.contains(&event_idx));
                        
                        if pattern_ref.negate {
                            !matched
                        } else {
                            matched
                        }
                    } else {
                        pattern_ref.negate
                    }
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_batch_processor() {
        // TODO: Add comprehensive tests
        let config = BatchProcessorConfig::default();
        let tiered_compiler = Arc::new(TieredCompiler::new());
        let processor = OptimizedBatchProcessor::new(config, tiered_compiler);
        
        // Test basic functionality
        assert_eq!(processor.config.batch_size, OPTIMAL_BATCH_SIZE);
    }
}
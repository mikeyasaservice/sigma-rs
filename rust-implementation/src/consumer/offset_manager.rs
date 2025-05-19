//! Manual offset management

use rdkafka::consumer::Consumer;
use rdkafka::error::KafkaError;
use rdkafka::TopicPartitionList;
use rdkafka::Offset;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// Manages Kafka offsets with batching and error handling
#[derive(Debug, Clone)]
pub struct OffsetManager {
    /// Pending offsets to commit
    pending_offsets: Arc<Mutex<HashMap<(String, i32), i64>>>,
    /// Committed offsets
    committed_offsets: Arc<Mutex<HashMap<(String, i32), i64>>>,
    /// Batch size for offset commits
    batch_size: usize,
    /// Maximum time between commits
    commit_interval: std::time::Duration,
}

impl OffsetManager {
    /// Create a new offset manager
    pub fn new(batch_size: usize, commit_interval: std::time::Duration) -> Self {
        Self {
            pending_offsets: Arc::new(Mutex::new(HashMap::new())),
            committed_offsets: Arc::new(Mutex::new(HashMap::new())),
            batch_size,
            commit_interval,
        }
    }
    
    /// Mark an offset for commit
    pub async fn mark_offset(&self, topic: String, partition: i32, offset: i64) {
        let mut pending = self.pending_offsets.lock().await;
        let topic_clone = topic.clone();
        pending.insert((topic, partition), offset);
        
        debug!(
            "Marked offset {} for topic {} partition {} for commit",
            offset, topic_clone, partition
        );
    }
    
    /// Commit pending offsets
    pub async fn commit_offsets<C: Consumer>(&self, consumer: &C) -> Result<(), KafkaError> {
        let mut pending = self.pending_offsets.lock().await;
        
        if pending.is_empty() {
            return Ok(());
        }
        
        let mut tpl = TopicPartitionList::new();
        
        for ((topic, partition), offset) in pending.iter() {
            tpl.add_partition_offset(topic, *partition, Offset::Offset(*offset + 1))?;
        }
        
        debug!("Committing {} offsets", pending.len());
        
        match consumer.commit(&tpl, rdkafka::consumer::CommitMode::Sync) {
            Ok(()) => {
                info!("Successfully committed {} offsets", pending.len());
                
                // Move to committed offsets
                let mut committed = self.committed_offsets.lock().await;
                for (key, value) in pending.drain() {
                    committed.insert(key, value);
                }
                
                Ok(())
            }
            Err(e) => {
                error!("Failed to commit offsets: {}", e);
                Err(e)
            }
        }
    }
    
    /// Get the last committed offset for a partition
    pub async fn get_committed_offset(&self, topic: &str, partition: i32) -> Option<i64> {
        let committed = self.committed_offsets.lock().await;
        committed.get(&(topic.to_string(), partition)).copied()
    }
    
    /// Get pending offsets count
    pub async fn pending_count(&self) -> usize {
        let pending = self.pending_offsets.lock().await;
        pending.len()
    }
    
    /// Check if we should commit based on batch size
    pub async fn should_commit(&self) -> bool {
        self.pending_count().await >= self.batch_size
    }
    
    /// Reset all offsets (useful for testing)
    pub async fn reset(&self) {
        let mut pending = self.pending_offsets.lock().await;
        let mut committed = self.committed_offsets.lock().await;
        pending.clear();
        committed.clear();
    }
    
    /// Get all pending offsets (for debugging)
    pub async fn get_pending_offsets(&self) -> HashMap<(String, i32), i64> {
        let pending = self.pending_offsets.lock().await;
        pending.clone()
    }
    
    /// Commit specific offsets
    pub async fn commit_specific<C: Consumer>(
        &self,
        consumer: &C,
        offsets: Vec<(String, i32, i64)>,
    ) -> Result<(), KafkaError> {
        let mut tpl = TopicPartitionList::new();
        
        for (topic, partition, offset) in &offsets {
            tpl.add_partition_offset(topic, *partition, Offset::Offset(*offset + 1))?;
        }
        
        consumer.commit(&tpl, rdkafka::consumer::CommitMode::Sync)?;
        
        // Update committed offsets
        let mut committed = self.committed_offsets.lock().await;
        for (topic, partition, offset) in offsets {
            committed.insert((topic, partition), offset);
        }
        
        Ok(())
    }
}

/// Offset commit strategy
#[derive(Debug, Clone, Copy)]
pub enum CommitStrategy {
    /// Commit after each message
    AfterEach,
    /// Commit after a batch of messages
    Batch(usize),
    /// Commit on a time interval
    Interval(std::time::Duration),
    /// Commit on batch size or interval, whichever comes first
    BatchOrInterval(usize, std::time::Duration),
}

impl CommitStrategy {
    /// Check if we should commit based on processed count
    pub fn should_commit(&self, processed_count: usize, last_commit: std::time::Instant) -> bool {
        match self {
            CommitStrategy::AfterEach => processed_count > 0,
            CommitStrategy::Batch(size) => processed_count >= *size,
            CommitStrategy::Interval(duration) => last_commit.elapsed() >= *duration,
            CommitStrategy::BatchOrInterval(size, duration) => {
                processed_count >= *size || last_commit.elapsed() >= *duration
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_offset_manager() {
        let manager = OffsetManager::new(10, std::time::Duration::from_secs(60));
        
        // Mark some offsets
        manager.mark_offset("test-topic".to_string(), 0, 100).await;
        manager.mark_offset("test-topic".to_string(), 1, 200).await;
        
        assert_eq!(manager.pending_count().await, 2);
        
        // Check pending offsets
        let pending = manager.get_pending_offsets().await;
        assert_eq!(pending.get(&("test-topic".to_string(), 0)), Some(&100));
        assert_eq!(pending.get(&("test-topic".to_string(), 1)), Some(&200));
    }
    
    #[test]
    fn test_commit_strategy() {
        let strategy = CommitStrategy::BatchOrInterval(100, std::time::Duration::from_secs(60));
        let start = std::time::Instant::now();
        
        assert!(!strategy.should_commit(50, start));
        assert!(strategy.should_commit(100, start));
    }
}
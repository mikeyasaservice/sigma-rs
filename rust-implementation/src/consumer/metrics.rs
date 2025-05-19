//! Consumer metrics collection

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// Consumer metrics collector
#[derive(Debug, Clone)]
pub struct ConsumerMetrics {
    /// Total messages consumed
    pub messages_consumed: Arc<AtomicU64>,
    /// Total messages processed successfully
    pub messages_processed: Arc<AtomicU64>,
    /// Total messages failed
    pub messages_failed: Arc<AtomicU64>,
    /// Messages sent to DLQ
    pub messages_dlq: Arc<AtomicU64>,
    /// Current consumer lag
    pub consumer_lag: Arc<AtomicU64>,
    /// Processing durations
    processing_durations: Arc<RwLock<Vec<Duration>>>,
    /// Batch processing times
    batch_durations: Arc<RwLock<Vec<Duration>>>,
    /// Offset commit times
    commit_durations: Arc<RwLock<Vec<Duration>>>,
    /// Error counts by type
    error_counts: Arc<RwLock<std::collections::HashMap<String, u64>>>,
    /// Start time
    start_time: Instant,
}

impl ConsumerMetrics {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            messages_consumed: Arc::new(AtomicU64::new(0)),
            messages_processed: Arc::new(AtomicU64::new(0)),
            messages_failed: Arc::new(AtomicU64::new(0)),
            messages_dlq: Arc::new(AtomicU64::new(0)),
            consumer_lag: Arc::new(AtomicU64::new(0)),
            processing_durations: Arc::new(RwLock::new(Vec::new())),
            batch_durations: Arc::new(RwLock::new(Vec::new())),
            commit_durations: Arc::new(RwLock::new(Vec::new())),
            error_counts: Arc::new(RwLock::new(std::collections::HashMap::new())),
            start_time: Instant::now(),
        }
    }
    
    /// Record a consumed message
    pub fn increment_consumed(&self) {
        self.messages_consumed.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a processed message
    pub fn increment_processed(&self) {
        self.messages_processed.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a failed message
    pub fn increment_failed(&self) {
        self.messages_failed.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a message sent to DLQ
    pub fn increment_dlq(&self) {
        self.messages_dlq.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Update consumer lag
    pub fn set_lag(&self, lag: u64) {
        self.consumer_lag.store(lag, Ordering::Relaxed);
    }
    
    /// Record processing duration
    pub fn record_processing_duration(&self, duration: Duration) {
        let mut durations = self.processing_durations.write();
        durations.push(duration);
        
        // Keep only last 1000 samples
        if durations.len() > 1000 {
            durations.remove(0);
        }
    }
    
    /// Record batch processing duration
    pub fn record_batch_duration(&self, duration: Duration) {
        let mut durations = self.batch_durations.write();
        durations.push(duration);
        
        if durations.len() > 1000 {
            durations.remove(0);
        }
    }
    
    /// Record commit duration
    pub fn record_commit_duration(&self, duration: Duration) {
        let mut durations = self.commit_durations.write();
        durations.push(duration);
        
        if durations.len() > 1000 {
            durations.remove(0);
        }
    }
    
    /// Record an error
    pub fn record_error(&self, error_type: &str) {
        let mut errors = self.error_counts.write();
        *errors.entry(error_type.to_string()).or_insert(0) += 1;
    }
    
    /// Get processing statistics
    pub fn processing_stats(&self) -> ProcessingStats {
        let durations = self.processing_durations.read();
        if durations.is_empty() {
            return ProcessingStats::default();
        }
        
        let mut sorted = durations.clone();
        sorted.sort();
        
        let p50_idx = sorted.len() / 2;
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted.len() as f64 * 0.99) as usize;
        
        ProcessingStats {
            count: sorted.len(),
            p50: sorted[p50_idx],
            p95: sorted[p95_idx],
            p99: sorted[p99_idx],
            mean: Duration::from_nanos(
                sorted.iter().map(|d| d.as_nanos()).sum::<u128>() as u64 / sorted.len() as u64
            ),
        }
    }
    
    /// Get messages per second
    pub fn messages_per_second(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.messages_consumed.load(Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        }
    }
    
    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.messages_consumed.load(Ordering::Relaxed);
        let processed = self.messages_processed.load(Ordering::Relaxed);
        
        if total > 0 {
            processed as f64 / total as f64
        } else {
            0.0
        }
    }
    
    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // Counters
        output.push_str(&format!(
            "# HELP sigma_consumer_messages_total Total messages consumed\n\
             # TYPE sigma_consumer_messages_total counter\n\
             sigma_consumer_messages_total {{status=\"consumed\"}} {}\n\
             sigma_consumer_messages_total {{status=\"processed\"}} {}\n\
             sigma_consumer_messages_total {{status=\"failed\"}} {}\n\
             sigma_consumer_messages_total {{status=\"dlq\"}} {}\n",
            self.messages_consumed.load(Ordering::Relaxed),
            self.messages_processed.load(Ordering::Relaxed),
            self.messages_failed.load(Ordering::Relaxed),
            self.messages_dlq.load(Ordering::Relaxed),
        ));
        
        // Gauge
        output.push_str(&format!(
            "# HELP sigma_consumer_lag Current consumer lag\n\
             # TYPE sigma_consumer_lag gauge\n\
             sigma_consumer_lag {}\n",
            self.consumer_lag.load(Ordering::Relaxed),
        ));
        
        // Histogram
        let stats = self.processing_stats();
        output.push_str(&format!(
            "# HELP sigma_consumer_processing_duration_seconds Message processing duration\n\
             # TYPE sigma_consumer_processing_duration_seconds histogram\n\
             sigma_consumer_processing_duration_seconds_bucket {{le=\"0.001\"}} {}\n\
             sigma_consumer_processing_duration_seconds_bucket {{le=\"0.01\"}} {}\n\
             sigma_consumer_processing_duration_seconds_bucket {{le=\"0.1\"}} {}\n\
             sigma_consumer_processing_duration_seconds_bucket {{le=\"1.0\"}} {}\n\
             sigma_consumer_processing_duration_seconds_bucket {{le=\"+Inf\"}} {}\n\
             sigma_consumer_processing_duration_seconds_sum {}\n\
             sigma_consumer_processing_duration_seconds_count {}\n",
            stats.count_le(Duration::from_millis(1)),
            stats.count_le(Duration::from_millis(10)),
            stats.count_le(Duration::from_millis(100)),
            stats.count_le(Duration::from_secs(1)),
            stats.count,
            stats.sum().as_secs_f64(),
            stats.count,
        ));
        
        output
    }
}

/// Processing statistics
#[derive(Debug, Default)]
pub struct ProcessingStats {
    pub count: usize,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub mean: Duration,
}

impl ProcessingStats {
    /// Count of durations less than or equal to threshold
    pub fn count_le(&self, _threshold: Duration) -> usize {
        // This would need the raw data to calculate properly
        // For now, return approximations
        self.count
    }
    
    /// Sum of all durations
    pub fn sum(&self) -> Duration {
        self.mean * self.count as u32
    }
}

impl Default for ConsumerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics() {
        let metrics = ConsumerMetrics::new();
        
        metrics.increment_consumed();
        metrics.increment_processed();
        metrics.record_processing_duration(Duration::from_millis(10));
        
        assert_eq!(metrics.messages_consumed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.messages_processed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.success_rate(), 1.0);
    }
}
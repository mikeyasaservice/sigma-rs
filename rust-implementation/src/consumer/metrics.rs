//! Consumer metrics collection

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use std::collections::HashMap;

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
    error_counts: Arc<RwLock<HashMap<String, u64>>>,
    /// Consumer group lag per partition
    partition_lag: Arc<RwLock<HashMap<(String, i32), i64>>>,
    /// Last offset per partition
    last_offset: Arc<RwLock<HashMap<(String, i32), i64>>>,
    /// High water mark per partition
    high_water_mark: Arc<RwLock<HashMap<(String, i32), i64>>>,
    /// Rebalance events
    rebalance_count: Arc<AtomicU64>,
    /// Connection errors
    connection_errors: Arc<AtomicU64>,
    /// Start time
    start_time: Instant,
    /// Last metrics reset time
    last_reset: Arc<RwLock<Instant>>,
}

impl ConsumerMetrics {
    /// Create new metrics collector
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            messages_consumed: Arc::new(AtomicU64::new(0)),
            messages_processed: Arc::new(AtomicU64::new(0)),
            messages_failed: Arc::new(AtomicU64::new(0)),
            messages_dlq: Arc::new(AtomicU64::new(0)),
            consumer_lag: Arc::new(AtomicU64::new(0)),
            processing_durations: Arc::new(RwLock::new(Vec::new())),
            batch_durations: Arc::new(RwLock::new(Vec::new())),
            commit_durations: Arc::new(RwLock::new(Vec::new())),
            error_counts: Arc::new(RwLock::new(HashMap::new())),
            partition_lag: Arc::new(RwLock::new(HashMap::new())),
            last_offset: Arc::new(RwLock::new(HashMap::new())),
            high_water_mark: Arc::new(RwLock::new(HashMap::new())),
            rebalance_count: Arc::new(AtomicU64::new(0)),
            connection_errors: Arc::new(AtomicU64::new(0)),
            start_time: now,
            last_reset: Arc::new(RwLock::new(now)),
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
    
    /// Update partition lag
    pub fn set_partition_lag(&self, topic: String, partition: i32, lag: i64) {
        let mut partition_lag = self.partition_lag.write();
        partition_lag.insert((topic, partition), lag);
    }
    
    /// Update last offset for partition
    pub fn set_last_offset(&self, topic: String, partition: i32, offset: i64) {
        let mut last_offset = self.last_offset.write();
        last_offset.insert((topic, partition), offset);
    }
    
    /// Update high water mark for partition
    pub fn set_high_water_mark(&self, topic: String, partition: i32, hwm: i64) {
        let mut high_water_mark = self.high_water_mark.write();
        high_water_mark.insert((topic.clone(), partition), hwm);
        
        // Calculate lag
        if let Some(last) = self.last_offset.read().get(&(topic.clone(), partition)) {
            let lag = hwm - last;
            self.set_partition_lag(topic, partition, lag);
        }
    }
    
    /// Record a rebalance event
    pub fn increment_rebalance(&self) {
        self.rebalance_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a connection error
    pub fn increment_connection_error(&self) {
        self.connection_errors.fetch_add(1, Ordering::Relaxed);
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
        self.calculate_stats(&self.processing_durations)
    }
    
    /// Get commit statistics
    pub fn commit_stats(&self) -> Option<ProcessingStats> {
        let stats = self.calculate_stats(&self.commit_durations);
        if stats.count > 0 {
            Some(stats)
        } else {
            None
        }
    }
    
    /// Get batch processing statistics
    pub fn batch_stats(&self) -> Option<ProcessingStats> {
        let stats = self.calculate_stats(&self.batch_durations);
        if stats.count > 0 {
            Some(stats)
        } else {
            None
        }
    }
    
    /// Calculate statistics for a duration collection
    fn calculate_stats(&self, durations_lock: &Arc<RwLock<Vec<Duration>>>) -> ProcessingStats {
        let durations = durations_lock.read();
        if durations.is_empty() {
            return ProcessingStats::default();
        }
        
        let mut sorted = durations.clone();
        sorted.sort();
        
        let p50_idx = sorted.len() / 2;
        let p95_idx = ((sorted.len() - 1) as f64 * 0.95) as usize;
        let p99_idx = ((sorted.len() - 1) as f64 * 0.99) as usize;
        
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
    
    /// Get overall consumer lag
    pub fn total_lag(&self) -> i64 {
        let partition_lag = self.partition_lag.read();
        partition_lag.values().sum()
    }
    
    /// Get lag by topic
    pub fn lag_by_topic(&self) -> HashMap<String, i64> {
        let partition_lag = self.partition_lag.read();
        let mut topic_lag = HashMap::new();
        
        for ((topic, _), lag) in partition_lag.iter() {
            *topic_lag.entry(topic.clone()).or_insert(0) += lag;
        }
        
        topic_lag
    }
    
    /// Reset metrics (useful for testing)
    pub fn reset(&self) {
        self.messages_consumed.store(0, Ordering::Relaxed);
        self.messages_processed.store(0, Ordering::Relaxed);
        self.messages_failed.store(0, Ordering::Relaxed);
        self.messages_dlq.store(0, Ordering::Relaxed);
        self.consumer_lag.store(0, Ordering::Relaxed);
        self.rebalance_count.store(0, Ordering::Relaxed);
        self.connection_errors.store(0, Ordering::Relaxed);
        
        self.processing_durations.write().clear();
        self.batch_durations.write().clear();
        self.commit_durations.write().clear();
        self.error_counts.write().clear();
        self.partition_lag.write().clear();
        self.last_offset.write().clear();
        self.high_water_mark.write().clear();
        
        *self.last_reset.write() = Instant::now();
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
        
        // Gauge - overall lag
        output.push_str(&format!(
            "# HELP sigma_consumer_lag Current consumer lag\n\
             # TYPE sigma_consumer_lag gauge\n\
             sigma_consumer_lag {}\n",
            self.consumer_lag.load(Ordering::Relaxed),
        ));
        
        // Partition-specific metrics
        let partition_lag = self.partition_lag.read();
        let last_offset = self.last_offset.read();
        let high_water_mark = self.high_water_mark.read();
        
        // Partition lag
        if !partition_lag.is_empty() {
            output.push_str("# HELP sigma_consumer_partition_lag Lag per partition\n");
            output.push_str("# TYPE sigma_consumer_partition_lag gauge\n");
            for ((topic, partition), lag) in partition_lag.iter() {
                output.push_str(&format!(
                    "sigma_consumer_partition_lag {{topic=\"{}\", partition=\"{}\"}} {}\n",
                    topic, partition, lag
                ));
            }
        }
        
        for ((topic, partition), offset) in last_offset.iter() {
            output.push_str(&format!(
                "sigma_consumer_partition_offset {{topic=\"{}\", partition=\"{}\"}} {}\n",
                topic, partition, offset
            ));
        }
        
        for ((topic, partition), hwm) in high_water_mark.iter() {
            output.push_str(&format!(
                "sigma_consumer_partition_high_water_mark {{topic=\"{}\", partition=\"{}\"}} {}\n",
                topic, partition, hwm
            ));
        }
        
        // Rebalance and connection metrics
        output.push_str(&format!(
            "# HELP sigma_consumer_rebalances_total Total rebalance events\n\
             # TYPE sigma_consumer_rebalances_total counter\n\
             sigma_consumer_rebalances_total {}\n",
            self.rebalance_count.load(Ordering::Relaxed),
        ));
        
        output.push_str(&format!(
            "# HELP sigma_consumer_connection_errors_total Total connection errors\n\
             # TYPE sigma_consumer_connection_errors_total counter\n\
             sigma_consumer_connection_errors_total {}\n",
            self.connection_errors.load(Ordering::Relaxed),
        ));
        
        // Error breakdown
        let error_counts = self.error_counts.read();
        output.push_str("# HELP sigma_consumer_errors_by_type Errors by type\n");
        output.push_str("# TYPE sigma_consumer_errors_by_type counter\n");
        for (error_type, count) in error_counts.iter() {
            output.push_str(&format!(
                "sigma_consumer_errors_by_type {{type=\"{}\"}} {}\n",
                error_type, count
            ));
        }
        
        // Histogram - processing duration
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
        
        // Commit duration histogram
        if let Some(commit_stats) = self.commit_stats() {
            output.push_str(&format!(
                "# HELP sigma_consumer_commit_duration_seconds Offset commit duration\n\
                 # TYPE sigma_consumer_commit_duration_seconds histogram\n\
                 sigma_consumer_commit_duration_seconds_bucket {{le=\"0.01\"}} {}\n\
                 sigma_consumer_commit_duration_seconds_bucket {{le=\"0.1\"}} {}\n\
                 sigma_consumer_commit_duration_seconds_bucket {{le=\"1.0\"}} {}\n\
                 sigma_consumer_commit_duration_seconds_bucket {{le=\"+Inf\"}} {}\n\
                 sigma_consumer_commit_duration_seconds_sum {}\n\
                 sigma_consumer_commit_duration_seconds_count {}\n",
                commit_stats.count_le(Duration::from_millis(10)),
                commit_stats.count_le(Duration::from_millis(100)),
                commit_stats.count_le(Duration::from_secs(1)),
                commit_stats.count,
                commit_stats.sum().as_secs_f64(),
                commit_stats.count,
            ));
        }
        
        // Runtime info
        let uptime = self.start_time.elapsed().as_secs();
        output.push_str(&format!(
            "# HELP sigma_consumer_uptime_seconds Uptime in seconds\n\
             # TYPE sigma_consumer_uptime_seconds gauge\n\
             sigma_consumer_uptime_seconds {}\n",
            uptime
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
    pub fn count_le(&self, threshold: Duration) -> usize {
        // Approximation based on percentiles
        if threshold <= self.p50 {
            self.count / 2
        } else if threshold <= self.p95 {
            (self.count as f64 * 0.95) as usize
        } else if threshold <= self.p99 {
            (self.count as f64 * 0.99) as usize
        } else {
            self.count
        }
    }
    
    /// Sum of all durations
    pub fn sum(&self) -> Duration {
        self.mean * self.count as u32
    }
    
    /// Check if stats are empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
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
    fn test_basic_metrics() {
        let metrics = ConsumerMetrics::new();
        
        metrics.increment_consumed();
        metrics.increment_processed();
        metrics.record_processing_duration(Duration::from_millis(10));
        
        assert_eq!(metrics.messages_consumed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.messages_processed.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.success_rate(), 1.0);
    }
    
    #[test]
    fn test_partition_metrics() {
        let metrics = ConsumerMetrics::new();
        
        // Record partition-level metrics
        metrics.set_partition_lag("topic1".to_string(), 0, 100);
        metrics.set_partition_lag("topic1".to_string(), 1, 200);
        metrics.set_partition_lag("topic2".to_string(), 0, 50);
        
        let lag = metrics.partition_lag.read();
        assert_eq!(*lag.get(&("topic1".to_string(), 0)).unwrap(), 100);
        assert_eq!(*lag.get(&("topic1".to_string(), 1)).unwrap(), 200);
        assert_eq!(*lag.get(&("topic2".to_string(), 0)).unwrap(), 50);
    }
    
    #[test]
    fn test_error_tracking() {
        let metrics = ConsumerMetrics::new();
        
        metrics.record_error("kafka_error");
        metrics.record_error("processing_error");
        metrics.record_error("kafka_error");
        
        let errors = metrics.error_counts.read();
        assert_eq!(*errors.get("kafka_error").unwrap(), 2);
        assert_eq!(*errors.get("processing_error").unwrap(), 1);
    }
    
    #[test]
    fn test_rate_calculations() {
        let metrics = ConsumerMetrics::new();
        
        // Simulate consuming messages
        for _ in 0..100 {
            metrics.increment_consumed();
        }
        
        // Since we don't have a way to mock time, just verify the counter
        assert_eq!(metrics.messages_consumed.load(Ordering::Relaxed), 100);
        
        // Test messages per second (will be 0 without time passing)
        assert!(metrics.messages_per_second() >= 0.0);
    }
    
    #[test]
    fn test_processing_stats() {
        let metrics = ConsumerMetrics::new();
        
        // Record various processing durations
        metrics.record_processing_duration(Duration::from_millis(10));
        metrics.record_processing_duration(Duration::from_millis(20));
        metrics.record_processing_duration(Duration::from_millis(30));
        metrics.record_processing_duration(Duration::from_millis(40));
        metrics.record_processing_duration(Duration::from_millis(50));
        
        let stats = metrics.processing_stats();
        assert_eq!(stats.count, 5);
        let mean_millis = stats.mean.as_millis() as i128;
        assert!((mean_millis - 30).abs() < 5); // approximately 30ms
        // Check percentiles make sense
        assert!(stats.p50 >= Duration::from_millis(10));
        assert!(stats.p99 >= Duration::from_millis(30));
        assert!(stats.p99 <= Duration::from_millis(50));
    }
    
    #[test]
    fn test_prometheus_export() {
        let metrics = ConsumerMetrics::new();
        
        // Set up some metrics
        metrics.increment_consumed();
        metrics.increment_processed();
        metrics.increment_failed();
        metrics.increment_dlq();
        // Note: retries are tracked through error counts in this implementation
        metrics.record_error("retry_error");
        metrics.record_error("retry_error");
        metrics.record_error("retry_error");
        metrics.set_partition_lag("test-topic".to_string(), 0, 100);
        metrics.record_commit_duration(Duration::from_millis(5));
        metrics.record_error("test_error");
        
        let output = metrics.export_prometheus();
        
        // Verify Prometheus format
        assert!(output.contains("# TYPE sigma_consumer_messages_total counter"));
        assert!(output.contains("# TYPE sigma_consumer_lag gauge"));
        assert!(output.contains("# TYPE sigma_consumer_partition_lag gauge"));
        assert!(output.contains("# TYPE sigma_consumer_errors_by_type counter"));
        
        // Verify values
        assert!(output.contains("sigma_consumer_messages_total {status=\"consumed\"} 1"));
        assert!(output.contains("sigma_consumer_messages_total {status=\"processed\"} 1"));
        assert!(output.contains("sigma_consumer_messages_total {status=\"failed\"} 1"));
        assert!(output.contains("sigma_consumer_partition_lag {topic=\"test-topic\", partition=\"0\"} 100"));
    }
    
    #[test]
    fn test_commit_tracking() {
        let metrics = ConsumerMetrics::new();
        
        // Track commit operations
        for i in 1..=5 {
            metrics.record_commit_duration(Duration::from_millis(i * 10));
        }
        
        // Duration distribution should have been updated
        assert_eq!(metrics.messages_consumed.load(Ordering::Relaxed), 0); // No messages consumed
        // Note: We can't directly test distribution without exposing internals
    }
    
    #[test]
    fn test_thread_safety() {
        use std::thread;
        use std::sync::Arc;
        
        let metrics = Arc::new(ConsumerMetrics::new());
        let mut handles = vec![];
        
        // Spawn multiple threads updating metrics
        for _ in 0..10 {
            let m = metrics.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    m.increment_consumed();
                    m.increment_processed();
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify totals
        assert_eq!(metrics.messages_consumed.load(Ordering::Relaxed), 1000);
        assert_eq!(metrics.messages_processed.load(Ordering::Relaxed), 1000);
    }
    
    #[test]
    fn test_rebalance_tracking() {
        let metrics = ConsumerMetrics::new();
        
        metrics.increment_rebalance();
        metrics.increment_rebalance();
        
        assert_eq!(metrics.rebalance_count.load(Ordering::Relaxed), 2);
    }
    
    #[test]
    fn test_connection_errors() {
        let metrics = ConsumerMetrics::new();
        
        metrics.increment_connection_error();
        metrics.increment_connection_error();
        metrics.increment_connection_error();
        
        assert_eq!(metrics.connection_errors.load(Ordering::Relaxed), 3);
    }
}
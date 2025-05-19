//! Backpressure control for the consumer

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, RwLock};
use tracing::{debug, warn, info};

/// Controls backpressure for message processing
#[derive(Debug, Clone)]
pub struct BackpressureController {
    /// Maximum number of inflight messages
    max_inflight: Arc<AtomicUsize>,
    /// Current inflight count
    inflight_count: Arc<AtomicUsize>,
    /// Semaphore for limiting concurrency
    semaphore: Arc<Semaphore>,
    /// Pause threshold (percentage)
    pause_threshold: f64,
    /// Resume threshold (percentage)
    resume_threshold: f64,
    /// Whether consumer is paused
    is_paused: Arc<AtomicUsize>, // Using AtomicUsize as AtomicBool is not as portable
    /// Memory usage tracking
    memory_limit: Option<usize>,
    /// Current memory usage (approximation)
    current_memory: Arc<AtomicUsize>,
    /// Average message size for memory estimation
    avg_message_size: Arc<AtomicUsize>,
    /// Metrics for adaptive adjustment
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
}

impl BackpressureController {
    /// Create a new backpressure controller
    pub fn new(
        max_inflight: usize,
        pause_threshold: f64,
        resume_threshold: f64,
    ) -> Self {
        Self {
            max_inflight: Arc::new(AtomicUsize::new(max_inflight)),
            inflight_count: Arc::new(AtomicUsize::new(0)),
            semaphore: Arc::new(Semaphore::new(max_inflight)),
            pause_threshold,
            resume_threshold,
            is_paused: Arc::new(AtomicUsize::new(0)),
            memory_limit: None,
            current_memory: Arc::new(AtomicUsize::new(0)),
            avg_message_size: Arc::new(AtomicUsize::new(1024)), // Default 1KB
            performance_metrics: Arc::new(RwLock::new(PerformanceMetrics::new())),
        }
    }
    
    /// Create with memory limit
    pub fn with_memory_limit(mut self, limit_mb: usize) -> Self {
        self.memory_limit = Some(limit_mb * 1024 * 1024);
        self
    }
    
    /// Update average message size estimate
    pub fn update_avg_message_size(&self, size: usize) {
        let current = self.avg_message_size.load(Ordering::Relaxed);
        // Exponential moving average
        let new_avg = (current * 9 + size) / 10;
        self.avg_message_size.store(new_avg, Ordering::Relaxed);
    }
    
    /// Acquire a permit for processing
    pub async fn acquire(&self) -> BackpressurePermit {
        // Check memory constraint first
        if let Some(limit) = self.memory_limit {
            loop {
                let current_mem = self.current_memory.load(Ordering::Relaxed);
                let avg_size = self.avg_message_size.load(Ordering::Relaxed);
                
                if current_mem + avg_size <= limit {
                    // Try to reserve memory
                    match self.current_memory.compare_exchange(
                        current_mem,
                        current_mem + avg_size,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(_) => continue, // Retry
                    }
                } else {
                    // Memory limit reached, wait
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
        
        let permit = self.semaphore.clone().acquire_owned().await
            .expect("Semaphore should not be closed");
        
        let count = self.inflight_count.fetch_add(1, Ordering::Relaxed) + 1;
        let max_inflight = self.max_inflight.load(Ordering::Relaxed);
        
        debug!("Acquired permit, inflight: {}/{}", count, max_inflight);
        
        BackpressurePermit {
            controller: self.clone(),
            _permit: permit,
            memory_reserved: self.avg_message_size.load(Ordering::Relaxed),
        }
    }
    
    /// Try to acquire a permit without blocking
    pub fn try_acquire(&self) -> Option<BackpressurePermit> {
        // Check memory constraint first
        if let Some(limit) = self.memory_limit {
            let current_mem = self.current_memory.load(Ordering::Relaxed);
            let avg_size = self.avg_message_size.load(Ordering::Relaxed);
            
            if current_mem + avg_size > limit {
                return None; // Memory limit would be exceeded
            }
        }
        
        match self.semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                // Reserve memory
                let memory_reserved = if self.memory_limit.is_some() {
                    let avg_size = self.avg_message_size.load(Ordering::Relaxed);
                    self.current_memory.fetch_add(avg_size, Ordering::Relaxed);
                    avg_size
                } else {
                    0
                };
                
                let count = self.inflight_count.fetch_add(1, Ordering::Relaxed) + 1;
                let max_inflight = self.max_inflight.load(Ordering::Relaxed);
                debug!("Try-acquired permit, inflight: {}/{}", count, max_inflight);
                
                Some(BackpressurePermit {
                    controller: self.clone(),
                    _permit: permit,
                    memory_reserved,
                })
            }
            Err(_) => None,
        }
    }
    
    /// Check if we should pause consumption
    pub fn should_pause(&self) -> bool {
        let inflight = self.inflight_count.load(Ordering::Relaxed);
        let max_inflight = self.max_inflight.load(Ordering::Relaxed);
        let threshold = (max_inflight as f64 * self.pause_threshold) as usize;
        
        if inflight >= threshold && self.is_paused.load(Ordering::Relaxed) == 0 {
            warn!("Backpressure threshold reached: {}/{}", inflight, max_inflight);
            self.is_paused.store(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
    
    /// Check if we should resume consumption
    pub fn should_resume(&self) -> bool {
        let inflight = self.inflight_count.load(Ordering::Relaxed);
        let max_inflight = self.max_inflight.load(Ordering::Relaxed);
        let threshold = (max_inflight as f64 * self.resume_threshold) as usize;
        
        if inflight <= threshold && self.is_paused.load(Ordering::Relaxed) == 1 {
            debug!("Resuming consumption, inflight: {}/{}", inflight, max_inflight);
            self.is_paused.store(0, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
    
    /// Record successful processing
    pub async fn record_success(&self, latency: Duration) {
        let mut metrics = self.performance_metrics.write().await;
        metrics.record_success(latency);
    }
    
    /// Record failed processing
    pub async fn record_failure(&self) {
        let mut metrics = self.performance_metrics.write().await;
        metrics.record_failure();
    }
    
    /// Record current throughput
    pub async fn record_throughput(&self, messages_per_second: f64) {
        let mut metrics = self.performance_metrics.write().await;
        metrics.record_throughput(messages_per_second);
    }
    
    /// Get current inflight count
    pub fn inflight_count(&self) -> usize {
        self.inflight_count.load(Ordering::Relaxed)
    }
    
    /// Check if consumer is paused
    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::Relaxed) == 1
    }
    
    /// Get percentage of capacity used
    pub fn utilization(&self) -> f64 {
        let inflight = self.inflight_count.load(Ordering::Relaxed);
        let max_inflight = self.max_inflight.load(Ordering::Relaxed);
        inflight as f64 / max_inflight as f64
    }
    
    /// Get current memory usage
    pub fn memory_usage(&self) -> usize {
        self.current_memory.load(Ordering::Relaxed)
    }
    
    /// Get memory utilization percentage
    pub fn memory_utilization(&self) -> Option<f64> {
        self.memory_limit.map(|limit| {
            self.current_memory.load(Ordering::Relaxed) as f64 / limit as f64
        })
    }
    
    /// Adjust max inflight based on performance
    pub async fn adjust_limits(&self) -> bool {
        let metrics = self.performance_metrics.read().await;
        
        // Only adjust if we have enough data
        if metrics.total_count < 100 {
            return false;
        }
        
        let current_limit = self.max_inflight.load(Ordering::Relaxed);
        let mut new_limit = current_limit;
        
        // Get performance metrics
        let success_rate = metrics.success_rate();
        let avg_latency = metrics.avg_latency();
        let p99_latency = metrics.p99_latency();
        
        // Decision logic
        if success_rate < 0.95 || p99_latency.map(|d| d > Duration::from_secs(5)).unwrap_or(false) {
            // Performance degraded, reduce concurrency
            new_limit = (current_limit as f64 * 0.9) as usize;
            new_limit = new_limit.max(1); // At least 1
        } else if success_rate > 0.99 && avg_latency.map(|d| d < Duration::from_millis(100)).unwrap_or(false) {
            // Performance good, increase concurrency
            new_limit = (current_limit as f64 * 1.1) as usize;
        }
        
        if new_limit != current_limit {
            info!(
                "Adjusting backpressure limit from {} to {} (success rate: {:.2}%, avg latency: {:?})",
                current_limit, new_limit, success_rate * 100.0, avg_latency
            );
            
            self.max_inflight.store(new_limit, Ordering::Relaxed);
            // Note: We can't resize the semaphore, so this is more of a soft limit
            return true;
        }
        
        false
    }
}

/// Permit for processing a message
#[derive(Debug)]
pub struct BackpressurePermit {
    controller: BackpressureController,
    #[allow(dead_code)]
    _permit: tokio::sync::OwnedSemaphorePermit,
    memory_reserved: usize,
}

impl Drop for BackpressurePermit {
    fn drop(&mut self) {
        let count = self.controller.inflight_count.fetch_sub(1, Ordering::Relaxed) - 1;
        let max_inflight = self.controller.max_inflight.load(Ordering::Relaxed);
        
        // Release memory
        if self.memory_reserved > 0 {
            self.controller.current_memory.fetch_sub(self.memory_reserved, Ordering::Relaxed);
        }
        
        debug!("Released permit, inflight: {}/{}", count, max_inflight);
    }
}

/// Performance metrics for adaptive backpressure
#[derive(Debug, Clone)]
struct PerformanceMetrics {
    /// Processing latencies
    latencies: Vec<Duration>,
    /// Success rate
    success_count: usize,
    /// Total count
    total_count: usize,
    /// Last adjustment time
    last_adjustment: Instant,
    /// Throughput samples (messages per second)
    throughput_samples: Vec<f64>,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            latencies: Vec::with_capacity(100),
            success_count: 0,
            total_count: 0,
            last_adjustment: Instant::now(),
            throughput_samples: Vec::with_capacity(10),
        }
    }
    
    fn record_success(&mut self, latency: Duration) {
        self.success_count += 1;
        self.total_count += 1;
        self.latencies.push(latency);
        
        // Keep only recent samples
        if self.latencies.len() > 100 {
            self.latencies.remove(0);
        }
    }
    
    fn record_failure(&mut self) {
        self.total_count += 1;
    }
    
    fn success_rate(&self) -> f64 {
        if self.total_count == 0 {
            1.0
        } else {
            self.success_count as f64 / self.total_count as f64
        }
    }
    
    fn avg_latency(&self) -> Option<Duration> {
        if self.latencies.is_empty() {
            None
        } else {
            let sum: Duration = self.latencies.iter().sum();
            Some(sum / self.latencies.len() as u32)
        }
    }
    
    fn p99_latency(&self) -> Option<Duration> {
        if self.latencies.is_empty() {
            None
        } else {
            let mut sorted = self.latencies.clone();
            sorted.sort();
            let idx = (sorted.len() as f64 * 0.99) as usize;
            Some(sorted[idx.min(sorted.len() - 1)])
        }
    }
    
    fn record_throughput(&mut self, messages_per_second: f64) {
        self.throughput_samples.push(messages_per_second);
        if self.throughput_samples.len() > 10 {
            self.throughput_samples.remove(0);
        }
    }
    
    fn avg_throughput(&self) -> f64 {
        if self.throughput_samples.is_empty() {
            0.0
        } else {
            self.throughput_samples.iter().sum::<f64>() / self.throughput_samples.len() as f64
        }
    }
}

/// Configuration for adaptive backpressure
#[derive(Debug, Clone)]
pub struct AdaptiveBackpressureConfig {
    pub initial_inflight: usize,
    pub min_inflight: usize,
    pub max_inflight: usize,
    pub pause_threshold: f64,
    pub resume_threshold: f64,
    pub adjustment_interval: Duration,
    pub target_latency: Duration,
    pub target_success_rate: f64,
}

impl Default for AdaptiveBackpressureConfig {
    fn default() -> Self {
        Self {
            initial_inflight: 100,
            min_inflight: 10,
            max_inflight: 1000,
            pause_threshold: 0.8,
            resume_threshold: 0.5,
            adjustment_interval: Duration::from_secs(30),
            target_latency: Duration::from_millis(100),
            target_success_rate: 0.95,
        }
    }
}

/// Adaptive backpressure controller that adjusts limits based on performance
#[derive(Debug)]
pub struct AdaptiveBackpressureController {
    base_controller: BackpressureController,
    min_inflight: usize,
    max_inflight: usize,
    adjustment_interval: Duration,
    target_latency: Duration,
    /// Target success rate
    target_success_rate: f64,
    /// Last metrics calculation time
    last_metrics_time: Arc<RwLock<Instant>>,
    /// Last message count for throughput calculation
    last_message_count: Arc<AtomicUsize>,
}

impl AdaptiveBackpressureController {
    /// Create a new adaptive controller
    pub fn new(
        initial_inflight: usize,
        min_inflight: usize,
        max_inflight: usize,
        pause_threshold: f64,
        resume_threshold: f64,
        adjustment_interval: Duration,
        target_latency: Duration,
        target_success_rate: f64,
    ) -> Self {
        Self {
            base_controller: BackpressureController::new(
                initial_inflight,
                pause_threshold,
                resume_threshold,
            ),
            min_inflight,
            max_inflight,
            adjustment_interval,
            target_latency,
            target_success_rate,
            last_metrics_time: Arc::new(RwLock::new(Instant::now())),
            last_message_count: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    /// Get the base controller
    pub fn controller(&self) -> &BackpressureController {
        &self.base_controller
    }
    
    /// Run periodic adjustment
    pub async fn run_adjustment_loop(&self) {
        let mut interval = tokio::time::interval(self.adjustment_interval);
        
        loop {
            interval.tick().await;
            
            // Calculate throughput
            let current_time = Instant::now();
            let last_time = *self.last_metrics_time.read().await;
            let elapsed = current_time.duration_since(last_time);
            
            if elapsed.as_secs() > 0 {
                let current_count = self.base_controller.inflight_count();
                let last_count = self.last_message_count.load(Ordering::Relaxed);
                let messages_per_second = (current_count.saturating_sub(last_count)) as f64 / elapsed.as_secs_f64();
                
                self.base_controller.record_throughput(messages_per_second).await;
                self.last_message_count.store(current_count, Ordering::Relaxed);
                *self.last_metrics_time.write().await = current_time;
            }
            
            // Adjust limits
            if self.base_controller.adjust_limits().await {
                // Limits were adjusted
                let new_limit = self.base_controller.max_inflight.load(Ordering::Relaxed);
                let clamped_limit = new_limit.clamp(self.min_inflight, self.max_inflight);
                self.base_controller.max_inflight.store(clamped_limit, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_backpressure_controller() {
        let controller = BackpressureController::new(10, 0.8, 0.5);
        
        // Acquire some permits
        let _permit1 = controller.acquire().await;
        let _permit2 = controller.acquire().await;
        
        assert_eq!(controller.inflight_count(), 2);
        assert!(!controller.should_pause());
        
        // Acquire more to trigger pause
        let mut permits = vec![];
        for _ in 0..6 {
            permits.push(controller.acquire().await);
        }
        
        assert!(controller.should_pause());
        assert!(controller.is_paused());
        
        // Drop some permits to trigger resume
        permits.clear();
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        assert!(controller.should_resume());
        assert!(!controller.is_paused());
    }
}
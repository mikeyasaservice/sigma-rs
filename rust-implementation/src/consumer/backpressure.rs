//! Backpressure control for the consumer

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, warn};

/// Controls backpressure for message processing
#[derive(Debug, Clone)]
pub struct BackpressureController {
    /// Maximum number of inflight messages
    max_inflight: usize,
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
}

impl BackpressureController {
    /// Create a new backpressure controller
    pub fn new(
        max_inflight: usize,
        pause_threshold: f64,
        resume_threshold: f64,
    ) -> Self {
        Self {
            max_inflight,
            inflight_count: Arc::new(AtomicUsize::new(0)),
            semaphore: Arc::new(Semaphore::new(max_inflight)),
            pause_threshold,
            resume_threshold,
            is_paused: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    /// Acquire a permit for processing
    pub async fn acquire(&self) -> BackpressurePermit {
        let permit = self.semaphore.clone().acquire_owned().await
            .expect("Semaphore should not be closed");
        
        let count = self.inflight_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        debug!("Acquired permit, inflight: {}/{}", count, self.max_inflight);
        
        BackpressurePermit {
            controller: self.clone(),
            _permit: permit,
        }
    }
    
    /// Try to acquire a permit without blocking
    pub fn try_acquire(&self) -> Option<BackpressurePermit> {
        match self.semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                let count = self.inflight_count.fetch_add(1, Ordering::Relaxed) + 1;
                debug!("Try-acquired permit, inflight: {}/{}", count, self.max_inflight);
                
                Some(BackpressurePermit {
                    controller: self.clone(),
                    _permit: permit,
                })
            }
            Err(_) => None,
        }
    }
    
    /// Check if we should pause consumption
    pub fn should_pause(&self) -> bool {
        let inflight = self.inflight_count.load(Ordering::Relaxed);
        let threshold = (self.max_inflight as f64 * self.pause_threshold) as usize;
        
        if inflight >= threshold && self.is_paused.load(Ordering::Relaxed) == 0 {
            warn!("Backpressure threshold reached: {}/{}", inflight, self.max_inflight);
            self.is_paused.store(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
    
    /// Check if we should resume consumption
    pub fn should_resume(&self) -> bool {
        let inflight = self.inflight_count.load(Ordering::Relaxed);
        let threshold = (self.max_inflight as f64 * self.resume_threshold) as usize;
        
        if inflight <= threshold && self.is_paused.load(Ordering::Relaxed) == 1 {
            debug!("Resuming consumption, inflight: {}/{}", inflight, self.max_inflight);
            self.is_paused.store(0, Ordering::Relaxed);
            true
        } else {
            false
        }
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
        inflight as f64 / self.max_inflight as f64
    }
}

/// Permit for processing a message
#[derive(Debug)]
pub struct BackpressurePermit {
    controller: BackpressureController,
    #[allow(dead_code)]
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl Drop for BackpressurePermit {
    fn drop(&mut self) {
        let count = self.controller.inflight_count.fetch_sub(1, Ordering::Relaxed) - 1;
        debug!("Released permit, inflight: {}/{}", count, self.controller.max_inflight);
    }
}

/// Adaptive backpressure controller that adjusts limits based on performance
#[derive(Debug)]
pub struct AdaptiveBackpressureController {
    base_controller: BackpressureController,
    min_inflight: usize,
    max_inflight: usize,
    adjustment_interval: std::time::Duration,
    target_latency: std::time::Duration,
}

impl AdaptiveBackpressureController {
    /// Create a new adaptive controller
    pub fn new(
        initial_inflight: usize,
        min_inflight: usize,
        max_inflight: usize,
        pause_threshold: f64,
        resume_threshold: f64,
        adjustment_interval: std::time::Duration,
        target_latency: std::time::Duration,
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
        }
    }
    
    /// Adjust limits based on current performance
    pub fn adjust_limits(&mut self, current_latency: std::time::Duration) {
        let current_limit = self.base_controller.max_inflight;
        let mut new_limit = current_limit;
        
        if current_latency > self.target_latency * 2 {
            // Latency too high, reduce concurrency
            new_limit = (current_limit as f64 * 0.9) as usize;
        } else if current_latency < self.target_latency / 2 {
            // Latency low, can increase concurrency
            new_limit = (current_limit as f64 * 1.1) as usize;
        }
        
        // Apply bounds
        new_limit = new_limit.clamp(self.min_inflight, self.max_inflight);
        
        if new_limit != current_limit {
            debug!(
                "Adjusting backpressure limit from {} to {} (latency: {:?})",
                current_limit, new_limit, current_latency
            );
            
            // This would require recreating the semaphore, which is not trivial
            // In practice, you might want to use a different approach
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
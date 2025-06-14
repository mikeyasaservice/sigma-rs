//! Graceful shutdown management for the consumer

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Manages the graceful shutdown state of the consumer
#[derive(Debug)]
pub struct ShutdownState {
    /// Whether shutdown has been initiated
    shutting_down: AtomicBool,
    /// Whether shutdown is complete
    shutdown_complete: AtomicBool,
    /// Number of in-flight messages
    inflight_messages: AtomicUsize,
    /// Shutdown initiated timestamp
    shutdown_start: tokio::sync::RwLock<Option<Instant>>,
}

impl ShutdownState {
    /// Create a new shutdown state
    pub fn new() -> Self {
        Self {
            shutting_down: AtomicBool::new(false),
            shutdown_complete: AtomicBool::new(false),
            inflight_messages: AtomicUsize::new(0),
            shutdown_start: tokio::sync::RwLock::new(None),
        }
    }

    /// Begin the shutdown process
    pub async fn begin_shutdown(&self) {
        self.shutting_down.store(true, Ordering::Relaxed);
        let mut shutdown_start = self.shutdown_start.write().await;
        *shutdown_start = Some(Instant::now());
        info!("Shutdown initiated");
    }

    /// Check if shutdown is in progress
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Relaxed)
    }

    /// Complete the shutdown process
    pub async fn complete_shutdown(&self) {
        self.shutdown_complete.store(true, Ordering::Relaxed);
        if let Some(start) = *self.shutdown_start.read().await {
            let duration = start.elapsed();
            info!("Shutdown completed in {:?}", duration);
        }
    }

    /// Check if shutdown is complete
    pub fn is_shutdown_complete(&self) -> bool {
        self.shutdown_complete.load(Ordering::Relaxed)
    }

    /// Add an in-flight message
    pub async fn add_inflight_message(&self) {
        let count = self.inflight_messages.fetch_add(1, Ordering::Relaxed) + 1;
        debug!("In-flight messages: {}", count);
    }

    /// Remove an in-flight message
    pub async fn remove_inflight_message(&self) {
        let count = self.inflight_messages.fetch_sub(1, Ordering::Relaxed);
        if count > 0 {
            debug!("In-flight messages: {}", count - 1);
        }
    }

    /// Check if there are any in-flight messages
    pub async fn has_inflight_messages(&self) -> bool {
        self.inflight_messages.load(Ordering::Relaxed) > 0
    }

    /// Get the count of in-flight messages
    pub async fn inflight_count(&self) -> usize {
        self.inflight_messages.load(Ordering::Relaxed)
    }

    /// Get the duration since shutdown started
    pub async fn shutdown_duration(&self) -> Option<Duration> {
        if let Some(start) = *self.shutdown_start.read().await {
            Some(start.elapsed())
        } else {
            None
        }
    }

    /// Start the shutdown process
    pub async fn start_shutdown(&self) {
        self.begin_shutdown().await;
    }

    /// Wait for all inflight messages to complete with timeout
    pub async fn wait_for_completion(&self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;

        while self.has_inflight_messages().await {
            if Instant::now() > deadline {
                let count = self.inflight_count().await;
                return Err(format!(
                    "Shutdown timeout with {} messages still in flight",
                    count
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        self.complete_shutdown().await;
        Ok(())
    }
}

impl Default for ShutdownState {
    fn default() -> Self {
        Self::new()
    }
}

/// Graceful shutdown coordinator
pub struct ShutdownCoordinator {
    /// Shutdown state
    state: Arc<ShutdownState>,
    /// Timeout for graceful shutdown
    timeout: Duration,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    pub fn new(state: Arc<ShutdownState>, timeout: Duration) -> Self {
        Self { state, timeout }
    }

    /// Execute graceful shutdown
    pub async fn shutdown(&self) -> Result<(), String> {
        // Begin shutdown
        self.state.begin_shutdown().await;

        // Wait for in-flight messages with timeout
        let deadline = Instant::now() + self.timeout;

        while self.state.has_inflight_messages().await {
            if Instant::now() > deadline {
                let count = self.state.inflight_count().await;
                warn!("Shutdown timeout with {} messages still in flight", count);
                return Err(format!("Timeout with {} messages in flight", count));
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Complete shutdown
        self.state.complete_shutdown().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_state() {
        let state = ShutdownState::new();

        assert!(!state.is_shutting_down());
        assert!(!state.is_shutdown_complete());

        state.begin_shutdown().await;
        assert!(state.is_shutting_down());

        state.add_inflight_message().await;
        assert!(state.has_inflight_messages().await);
        assert_eq!(state.inflight_count().await, 1);

        state.remove_inflight_message().await;
        assert!(!state.has_inflight_messages().await);

        state.complete_shutdown().await;
        assert!(state.is_shutdown_complete());
    }

    #[tokio::test]
    async fn test_shutdown_coordinator() {
        let state = Arc::new(ShutdownState::new());
        let coordinator = ShutdownCoordinator::new(state.clone(), Duration::from_secs(1));

        // Simulate in-flight messages
        state.add_inflight_message().await;

        // Start shutdown in background
        let shutdown_handle = tokio::spawn(async move { coordinator.shutdown().await });

        // Simulate message processing completion
        tokio::time::sleep(Duration::from_millis(100)).await;
        state.remove_inflight_message().await;

        // Shutdown should complete successfully
        let result = shutdown_handle.await.unwrap();
        assert!(result.is_ok());
        assert!(state.is_shutdown_complete());
    }
}

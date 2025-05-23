//! Retry logic and policies for message processing

use std::time::Duration;
use tracing::{debug, warn};
use serde::{Deserialize, Deserializer};

/// Maximum allowed retry attempts to prevent DoS
const MAX_RETRY_ATTEMPTS: u32 = 1000;
/// Maximum backoff duration to prevent excessive delays
const MAX_BACKOFF_SECONDS: u64 = 3600; // 1 hour
/// Maximum multiplier to prevent exponential explosion
const MAX_MULTIPLIER: f64 = 100.0;
/// Maximum jitter factor
const MAX_JITTER_FACTOR: f64 = 1.0;

/// Validate retry count within reasonable bounds
fn validate_max_retries<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u32::deserialize(deserializer)?;
    if value > MAX_RETRY_ATTEMPTS {
        return Err(serde::de::Error::custom(format!(
            "max_retries {} exceeds maximum allowed value {}",
            value, MAX_RETRY_ATTEMPTS
        )));
    }
    Ok(value)
}

/// Validate backoff duration within reasonable bounds
fn validate_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let duration = Duration::deserialize(deserializer)?;
    if duration.as_secs() > MAX_BACKOFF_SECONDS {
        return Err(serde::de::Error::custom(format!(
            "duration {:?} exceeds maximum allowed {} seconds",
            duration, MAX_BACKOFF_SECONDS
        )));
    }
    Ok(duration)
}

/// Validate multiplier within reasonable bounds
fn validate_multiplier<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    if !value.is_finite() || value < 0.0 || value > MAX_MULTIPLIER {
        return Err(serde::de::Error::custom(format!(
            "backoff_multiplier {} must be finite and between 0.0 and {}",
            value, MAX_MULTIPLIER
        )));
    }
    Ok(value)
}

/// Validate jitter factor within bounds [0.0, 1.0]
fn validate_jitter<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    if !value.is_finite() || value < 0.0 || value > MAX_JITTER_FACTOR {
        return Err(serde::de::Error::custom(format!(
            "jitter_factor {} must be finite and between 0.0 and {}",
            value, MAX_JITTER_FACTOR
        )));
    }
    Ok(value)
}

/// Retry policy configuration with validated bounds
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retries
    #[serde(deserialize_with = "validate_max_retries")]
    pub max_retries: u32,
    /// Initial backoff duration
    #[serde(deserialize_with = "validate_duration")]
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    #[serde(deserialize_with = "validate_duration")]
    pub max_backoff: Duration,
    /// Backoff multiplier (e.g., 2.0 for exponential)
    #[serde(deserialize_with = "validate_multiplier")]
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    #[serde(deserialize_with = "validate_jitter")]
    pub jitter_factor: f64,
    /// Whether to use exponential backoff
    pub exponential: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
            exponential: true,
        }
    }
}

impl RetryPolicy {
    /// Calculate the next backoff duration
    pub fn next_backoff(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        
        let base_backoff = if self.exponential {
            // Use safe multiplier calculation to prevent overflow
            let safe_multiplier = if attempt > 30 {
                // For very high attempts, just use max backoff
                self.max_backoff.as_secs_f64() / self.initial_backoff.as_secs_f64()
            } else {
                // Calculate power safely using f64
                let multiplier = self.backoff_multiplier.powf((attempt as i32 - 1) as f64);
                // Ensure multiplier is finite and reasonable
                if multiplier.is_finite() && multiplier < 1e6 {
                    multiplier
                } else {
                    self.max_backoff.as_secs_f64() / self.initial_backoff.as_secs_f64()
                }
            };
            Duration::from_secs_f64(self.initial_backoff.as_secs_f64() * safe_multiplier)
        } else {
            self.initial_backoff
        };
        
        // Cap at maximum backoff
        let capped_backoff = base_backoff.min(self.max_backoff);
        
        // Add jitter
        let jitter = capped_backoff.as_secs_f64() * self.jitter_factor * rand::random::<f64>();
        let with_jitter = Duration::from_secs_f64(capped_backoff.as_secs_f64() + jitter);
        
        debug!(
            "Calculated backoff for attempt {}: {:?} (base: {:?})",
            attempt, with_jitter, base_backoff
        );
        
        with_jitter
    }
    
    /// Check if we should retry
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
    
    /// Create a policy with no retries
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }
    
    /// Create a policy with fixed backoff
    pub fn fixed(max_retries: u32, backoff: Duration) -> Self {
        Self {
            max_retries,
            initial_backoff: backoff,
            max_backoff: backoff,
            exponential: false,
            backoff_multiplier: 1.0,
            jitter_factor: 0.0,
        }
    }
    
    /// Create a policy with exponential backoff
    pub fn exponential(max_retries: u32, initial: Duration, max: Duration) -> Self {
        Self {
            max_retries,
            initial_backoff: initial,
            max_backoff: max,
            exponential: true,
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

/// Retry result
#[derive(Debug)]
pub enum RetryResult<T, E> {
    /// Success after retries
    Success { value: T, attempts: u32 },
    /// Failed after exhausting retries
    Failed { error: E, attempts: u32 },
}

/// Retry executor
pub struct RetryExecutor {
    policy: RetryPolicy,
}

impl RetryExecutor {
    /// Create a new retry executor
    pub fn new(policy: RetryPolicy) -> Self {
        Self { policy }
    }
    
    /// Execute a function with retry logic
    pub async fn execute<F, Fut, T, E>(
        &self,
        mut operation: F,
    ) -> RetryResult<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut attempt = 0;
        
        loop {
            match operation().await {
                Ok(value) => {
                    if attempt > 0 {
                        debug!("Operation succeeded after {} retries", attempt);
                    }
                    return RetryResult::Success { value, attempts: attempt };
                }
                Err(error) => {
                    if !self.policy.should_retry(attempt) {
                        warn!(
                            "Operation failed after {} attempts: {}",
                            attempt + 1,
                            error
                        );
                        return RetryResult::Failed { error, attempts: attempt };
                    }
                    
                    attempt += 1;
                    let backoff = self.policy.next_backoff(attempt);
                    
                    warn!(
                        "Operation failed (attempt {}), retrying in {:?}: {}",
                        attempt, backoff, error
                    );
                    
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
    
    /// Execute with a custom retry predicate
    pub async fn execute_with_predicate<F, Fut, P, T, E>(
        &self,
        mut operation: F,
        mut should_retry: P,
    ) -> RetryResult<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        P: FnMut(&E) -> bool,
        E: std::fmt::Display,
    {
        let mut attempt = 0;
        
        loop {
            match operation().await {
                Ok(value) => {
                    if attempt > 0 {
                        debug!("Operation succeeded after {} retries", attempt);
                    }
                    return RetryResult::Success { value, attempts: attempt };
                }
                Err(error) => {
                    if !self.policy.should_retry(attempt) || !should_retry(&error) {
                        warn!(
                            "Operation failed after {} attempts: {}",
                            attempt + 1,
                            error
                        );
                        return RetryResult::Failed { error, attempts: attempt };
                    }
                    
                    attempt += 1;
                    let backoff = self.policy.next_backoff(attempt);
                    
                    warn!(
                        "Operation failed (attempt {}), retrying in {:?}: {}",
                        attempt, backoff, error
                    );
                    
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exponential_backoff() {
        let policy = RetryPolicy::exponential(5, Duration::from_millis(100), Duration::from_secs(10));
        
        // First attempt has no backoff
        assert_eq!(policy.next_backoff(0), Duration::ZERO);
        
        // Subsequent attempts have exponential backoff
        let backoff1 = policy.next_backoff(1);
        assert!(backoff1 >= Duration::from_millis(100));
        assert!(backoff1 < Duration::from_millis(200)); // With jitter
        
        let backoff2 = policy.next_backoff(2);
        assert!(backoff2 >= Duration::from_millis(200));
        assert!(backoff2 < Duration::from_millis(400)); // With jitter
    }
    
    #[test]
    fn test_fixed_backoff() {
        let policy = RetryPolicy::fixed(3, Duration::from_millis(500));
        
        assert_eq!(policy.next_backoff(0), Duration::ZERO);
        assert_eq!(policy.next_backoff(1), Duration::from_millis(500));
        assert_eq!(policy.next_backoff(2), Duration::from_millis(500));
        assert_eq!(policy.next_backoff(3), Duration::from_millis(500));
    }
    
    #[test]
    fn test_should_retry() {
        let policy = RetryPolicy::default();
        
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3)); // max_retries is 3
    }
    
    #[tokio::test]
    async fn test_retry_executor() {
        let policy = RetryPolicy::fixed(2, Duration::from_millis(10));
        let executor = RetryExecutor::new(policy);
        
        let count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let count_clone = count.clone();
        
        let result = executor.execute(move || {
            let count = count_clone.clone();
            async move {
                let current = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if current < 2 {
                    Err("temporary error")
                } else {
                    Ok("success")
                }
            }
        }).await;
        
        match result {
            RetryResult::Success { value, attempts } => {
                assert_eq!(value, "success");
                assert_eq!(attempts, 2);
            }
            _ => panic!("Expected success"),
        }
    }
    
    #[test]
    fn test_retry_policy_validation() {
        // Test valid configuration
        let valid_json = r#"
        {
            "max_retries": 5,
            "initial_backoff": {"secs": 1, "nanos": 0},
            "max_backoff": {"secs": 30, "nanos": 0},
            "backoff_multiplier": 2.0,
            "jitter_factor": 0.1,
            "exponential": true
        }"#;
        
        let policy: Result<RetryPolicy, _> = serde_json::from_str(valid_json);
        assert!(policy.is_ok());
        
        // Test invalid max_retries (too high)
        let invalid_retries_json = r#"
        {
            "max_retries": 10000,
            "initial_backoff": {"secs": 1, "nanos": 0},
            "max_backoff": {"secs": 30, "nanos": 0},
            "backoff_multiplier": 2.0,
            "jitter_factor": 0.1,
            "exponential": true
        }"#;
        
        let policy: Result<RetryPolicy, _> = serde_json::from_str(invalid_retries_json);
        assert!(policy.is_err());
        
        // Test invalid backoff duration (too long)
        let invalid_duration_json = r#"
        {
            "max_retries": 5,
            "initial_backoff": {"secs": 7200, "nanos": 0},
            "max_backoff": {"secs": 30, "nanos": 0},
            "backoff_multiplier": 2.0,
            "jitter_factor": 0.1,
            "exponential": true
        }"#;
        
        let policy: Result<RetryPolicy, _> = serde_json::from_str(invalid_duration_json);
        assert!(policy.is_err());
        
        // Test invalid multiplier (negative)
        let invalid_multiplier_json = r#"
        {
            "max_retries": 5,
            "initial_backoff": {"secs": 1, "nanos": 0},
            "max_backoff": {"secs": 30, "nanos": 0},
            "backoff_multiplier": -1.0,
            "jitter_factor": 0.1,
            "exponential": true
        }"#;
        
        let policy: Result<RetryPolicy, _> = serde_json::from_str(invalid_multiplier_json);
        assert!(policy.is_err());
        
        // Test invalid jitter factor (> 1.0)
        let invalid_jitter_json = r#"
        {
            "max_retries": 5,
            "initial_backoff": {"secs": 1, "nanos": 0},
            "max_backoff": {"secs": 30, "nanos": 0},
            "backoff_multiplier": 2.0,
            "jitter_factor": 1.5,
            "exponential": true
        }"#;
        
        let policy: Result<RetryPolicy, _> = serde_json::from_str(invalid_jitter_json);
        assert!(policy.is_err());
    }
}
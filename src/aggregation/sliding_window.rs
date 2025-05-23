use std::time::Duration;
use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// Production sliding window implementation for time-based aggregations
#[derive(Debug)]
pub struct SlidingWindow {
    /// Duration of the sliding window
    duration: Duration,
    /// Internal window state
    inner: RwLock<WindowInner>,
}

#[derive(Debug)]
struct WindowInner {
    /// Time-ordered entries in the window
    entries: VecDeque<WindowEntry>,
    /// Cached aggregated value
    cached_value: f64,
    /// Last update timestamp
    last_update: DateTime<Utc>,
    /// Whether cache is valid
    cache_valid: bool,
}

#[derive(Debug, Clone)]
struct WindowEntry {
    value: f64,
    timestamp: DateTime<Utc>,
}

impl SlidingWindow {
    /// Create a new sliding window with specified duration
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            inner: RwLock::new(WindowInner {
                entries: VecDeque::new(),
                cached_value: 0.0,
                last_update: Utc::now(),
                cache_valid: false,
            }),
        }
    }
    
    /// Add a value to the sliding window at the specified timestamp
    pub fn add_value(&self, value: f64, timestamp: DateTime<Utc>) {
        let mut inner = self.inner.write();
        
        // Add new entry
        inner.entries.push_back(WindowEntry { value, timestamp });
        
        // Remove entries outside the window
        let window_start = timestamp - chrono::Duration::from_std(self.duration).unwrap_or_default();
        while let Some(front) = inner.entries.front() {
            if front.timestamp >= window_start {
                break;
            }
            inner.entries.pop_front();
        }
        
        // Invalidate cache since we added new data
        inner.cache_valid = false;
        inner.last_update = timestamp;
    }
    
    /// Get current aggregated value in the window
    pub fn get_current_value(&self) -> f64 {
        let mut inner = self.inner.write();
        
        if !inner.cache_valid {
            // Recalculate aggregated value with overflow protection
            inner.cached_value = inner.entries.iter()
                .map(|e| e.value)
                .fold(0.0, |acc, val| {
                    let sum = acc + val;
                    if sum.is_finite() { sum } else { f64::MAX }
                });
            inner.cache_valid = true;
        }
        
        inner.cached_value
    }
    
    /// Get interpolated value at a specific timestamp
    pub fn get_interpolated_value(&self, timestamp: DateTime<Utc>) -> f64 {
        let inner = self.inner.read();
        
        // First, clean up entries outside the window from the query timestamp
        let window_start = timestamp - chrono::Duration::from_std(self.duration).unwrap_or_default();
        
        // Calculate value based on entries that would be valid at the given timestamp
        let interpolated_value: f64 = inner.entries
            .iter()
            .filter(|entry| entry.timestamp >= window_start && entry.timestamp <= timestamp)
            .map(|entry| entry.value)
            .fold(0.0, |acc, val| {
                let sum = acc + val;
                if sum.is_finite() { sum } else { f64::MAX }
            });
        
        interpolated_value
    }
    
    /// Get the number of entries currently in the window
    pub fn entry_count(&self) -> usize {
        let inner = self.inner.read();
        inner.entries.len()
    }
    
    /// Get the time range currently covered by the window
    pub fn time_range(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        let inner = self.inner.read();
        match (inner.entries.front(), inner.entries.back()) {
            (Some(first), Some(last)) => Some((first.timestamp, last.timestamp)),
            _ => None,
        }
    }
    
    /// Clear all entries from the window
    pub fn clear(&self) {
        let mut inner = self.inner.write();
        inner.entries.clear();
        inner.cached_value = 0.0;
        inner.cache_valid = true; // Empty window has valid cache of 0
        inner.last_update = Utc::now();
    }
    
    /// Get window duration
    pub fn duration(&self) -> Duration {
        self.duration
    }
    
    /// Compact the window by removing expired entries based on current time
    pub fn compact(&self) {
        self.compact_at_time(Utc::now())
    }
    
    /// Compact the window by removing entries expired as of the given time
    pub fn compact_at_time(&self, now: DateTime<Utc>) {
        let mut inner = self.inner.write();
        
        let window_start = now - chrono::Duration::from_std(self.duration).unwrap_or_default();
        let mut removed_any = false;
        
        while let Some(front) = inner.entries.front() {
            if front.timestamp >= window_start {
                break;
            }
            inner.entries.pop_front();
            removed_any = true;
        }
        
        if removed_any {
            inner.cache_valid = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    
    #[test]
    fn test_sliding_window_basic() {
        let window = SlidingWindow::new(Duration::from_secs(10));
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        
        // Add values
        window.add_value(1.0, base_time);
        window.add_value(2.0, base_time + chrono::Duration::seconds(5));
        window.add_value(3.0, base_time + chrono::Duration::seconds(10));
        
        assert_eq!(window.get_current_value(), 6.0);
        assert_eq!(window.entry_count(), 3);
    }
    
    #[test]
    fn test_sliding_window_expiration() {
        let window = SlidingWindow::new(Duration::from_secs(5));
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        
        // Add values
        window.add_value(1.0, base_time);
        window.add_value(2.0, base_time + chrono::Duration::seconds(3));
        window.add_value(3.0, base_time + chrono::Duration::seconds(6));
        
        // First value should be expired
        assert_eq!(window.entry_count(), 2);
        assert_eq!(window.get_current_value(), 5.0);
    }
    
    #[test]
    fn test_sliding_window_arithmetic_overflow() {
        let window = SlidingWindow::new(Duration::from_secs(10));
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        
        // Add values that would overflow
        window.add_value(f64::MAX / 2.0, base_time);
        window.add_value(f64::MAX / 2.0, base_time + chrono::Duration::seconds(1));
        window.add_value(f64::MAX / 2.0, base_time + chrono::Duration::seconds(2));
        
        // Should handle overflow gracefully
        let value = window.get_current_value();
        assert!(value.is_finite());
        assert_eq!(value, f64::MAX);
    }
    
    #[test]
    fn test_sliding_window_nan_handling() {
        let window = SlidingWindow::new(Duration::from_secs(10));
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        
        // Add NaN value
        window.add_value(1.0, base_time);
        window.add_value(f64::NAN, base_time + chrono::Duration::seconds(1));
        window.add_value(2.0, base_time + chrono::Duration::seconds(2));
        
        // NaN should propagate but be handled
        let value = window.get_current_value();
        assert!(!value.is_finite() || value == f64::MAX);
    }
    
    #[test]
    fn test_sliding_window_interpolation() {
        let window = SlidingWindow::new(Duration::from_secs(10));
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        
        // Add values
        window.add_value(1.0, base_time);
        window.add_value(2.0, base_time + chrono::Duration::seconds(5));
        
        // At time 5, window should include values from -5 to 5
        let value_at_5 = window.get_interpolated_value(base_time + chrono::Duration::seconds(5));
        assert_eq!(value_at_5, 3.0); // 1.0 + 2.0
        
        // Add value at time 15 - this will remove value at time 0
        window.add_value(3.0, base_time + chrono::Duration::seconds(15));
        
        // At time 15, window should include values from 5 to 15
        let value_at_15 = window.get_interpolated_value(base_time + chrono::Duration::seconds(15));
        assert_eq!(value_at_15, 5.0); // 2.0 + 3.0
        
        // Now if we query at time 5 again, value at 0 should be gone
        let value_at_5_after = window.get_interpolated_value(base_time + chrono::Duration::seconds(5));
        assert_eq!(value_at_5_after, 2.0); // Only 2.0, since 1.0 was removed
    }
    
    #[test]
    fn test_sliding_window_compact() {
        let window = SlidingWindow::new(Duration::from_secs(5));
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        
        // Add values
        window.add_value(1.0, base_time);
        window.add_value(2.0, base_time + chrono::Duration::seconds(2));
        window.add_value(3.0, base_time + chrono::Duration::seconds(4));
        
        assert_eq!(window.entry_count(), 3);
        
        // Compact at a future time
        window.compact_at_time(base_time + chrono::Duration::seconds(10));
        
        // All entries should be removed
        assert_eq!(window.entry_count(), 0);
        assert_eq!(window.get_current_value(), 0.0);
    }
    
    #[test]
    fn test_sliding_window_time_range() {
        let window = SlidingWindow::new(Duration::from_secs(10));
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        
        // Empty window
        assert!(window.time_range().is_none());
        
        // Add values
        window.add_value(1.0, base_time);
        window.add_value(2.0, base_time + chrono::Duration::seconds(5));
        
        let range = window.time_range().unwrap();
        assert_eq!(range.0, base_time);
        assert_eq!(range.1, base_time + chrono::Duration::seconds(5));
    }
    
    #[test]
    fn test_sliding_window_clear() {
        let window = SlidingWindow::new(Duration::from_secs(10));
        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        
        // Add values
        window.add_value(1.0, base_time);
        window.add_value(2.0, base_time + chrono::Duration::seconds(5));
        
        assert_eq!(window.entry_count(), 2);
        assert_eq!(window.get_current_value(), 3.0);
        
        window.clear();
        
        assert_eq!(window.entry_count(), 0);
        assert_eq!(window.get_current_value(), 0.0);
    }
}
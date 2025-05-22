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
            // Recalculate aggregated value
            inner.cached_value = inner.entries.iter().map(|e| e.value).sum();
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
            .sum();
        
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
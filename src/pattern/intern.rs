//! String interning for pattern matching to reduce allocations
//!
//! This module provides a thread-safe string interner that allows multiple
//! patterns to share the same string storage, reducing memory overhead
//! and improving cache locality.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};
use once_cell::sync::Lazy;

/// Global string interner for pattern strings
static PATTERN_INTERNER: Lazy<StringInterner> = Lazy::new(StringInterner::new);

/// Configuration for the string interner
pub struct StringInternerConfig {
    /// Maximum number of entries to cache
    pub max_capacity: usize,
}

impl Default for StringInternerConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10_000, // Default to 10k entries
        }
    }
}

/// Thread-safe string interner with size limits
pub struct StringInterner {
    strings: RwLock<HashMap<String, Arc<str>>>,
    max_capacity: usize,
    poison_count: AtomicU64,
}

impl StringInterner {
    /// Create a new string interner with default configuration
    pub fn new() -> Self {
        Self::with_config(StringInternerConfig::default())
    }
    
    /// Create a new string interner with specified configuration
    pub fn with_config(config: StringInternerConfig) -> Self {
        Self {
            strings: RwLock::new(HashMap::with_capacity(config.max_capacity)),
            max_capacity: config.max_capacity,
            poison_count: AtomicU64::new(0),
        }
    }
    
    /// Intern a string, returning a shared reference
    pub fn intern(&self, s: &str) -> Arc<str> {
        // First try to get from read lock (fast path)
        match self.strings.read() {
            Ok(strings) => {
                if let Some(interned) = strings.get(s) {
                    return Arc::clone(interned);
                }
            }
            Err(poisoned) => {
                // Log error and clear the interner to recover from poison
                tracing::error!("StringInterner read lock was poisoned - thread panic occurred");
                self.poison_count.fetch_add(1, Ordering::Relaxed);
                // Clear and continue with fresh state
                drop(poisoned);
            }
        }
        
        // Need to insert, acquire write lock
        let mut strings = match self.strings.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // Log error and recover by clearing the interner
                tracing::error!("StringInterner write lock was poisoned - thread panic occurred");
                self.poison_count.fetch_add(1, Ordering::Relaxed);
                // Extract the inner data and clear it
                let mut guard = poisoned.into_inner();
                guard.clear();
                guard
            }
        };
        
        // Check again in case another thread inserted while we waited
        if let Some(interned) = strings.get(s) {
            return Arc::clone(interned);
        }
        
        // Check if we need to evict old entries
        if strings.len() >= self.max_capacity {
            // Simple eviction: remove ~10% of entries
            // In production, you'd want a proper LRU implementation
            let to_remove = self.max_capacity / 10;
            let keys_to_remove: Vec<String> = strings.keys()
                .take(to_remove)
                .cloned()
                .collect();
            for key in keys_to_remove {
                strings.remove(&key);
            }
            tracing::warn!("StringInterner capacity reached, evicted {} entries", to_remove);
        }
        
        // Insert new string
        let interned: Arc<str> = Arc::from(s);
        strings.insert(s.to_string(), Arc::clone(&interned));
        interned
    }
    
    /// Get statistics about the interner
    pub fn stats(&self) -> InternerStats {
        match self.strings.read() {
            Ok(strings) => {
                let unique_strings = strings.len();
                // Estimate memory saved by counting reference counts
                // Note: Arc::strong_count is approximate in concurrent scenarios
                let estimated_memory_saved: usize = strings.iter()
                    .map(|(k, v)| {
                        let ref_count = Arc::strong_count(v).saturating_sub(1);
                        k.len() * ref_count
                    })
                    .sum();
                    
                InternerStats {
                    unique_strings,
                    estimated_memory_saved,
                    poison_events: self.poison_count.load(Ordering::Relaxed),
                }
            }
            Err(_) => {
                // If poisoned, return empty stats
                tracing::error!("StringInterner read lock was poisoned in stats()");
                self.poison_count.fetch_add(1, Ordering::Relaxed);
                InternerStats {
                    unique_strings: 0,
                    estimated_memory_saved: 0,
                    poison_events: self.poison_count.load(Ordering::Relaxed),
                }
            }
        }
    }
}

/// Statistics about string interning
#[derive(Debug, Clone)]
pub struct InternerStats {
    /// Number of unique strings
    pub unique_strings: usize,
    /// Estimated memory saved by interning (bytes)
    pub estimated_memory_saved: usize,
    /// Number of poison recovery events (for monitoring)
    pub poison_events: u64,
}

/// Intern a pattern string using the global interner
pub fn intern_pattern(s: &str) -> Arc<str> {
    PATTERN_INTERNER.intern(s)
}

/// Get global interner statistics
pub fn global_interner_stats() -> InternerStats {
    PATTERN_INTERNER.stats()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_string_interner() {
        let interner = StringInterner::new();
        
        // Intern the same string multiple times
        let s1 = interner.intern("test");
        let s2 = interner.intern("test");
        let s3 = interner.intern("different");
        
        // Should be the same Arc
        assert!(Arc::ptr_eq(&s1, &s2));
        assert!(!Arc::ptr_eq(&s1, &s3));
        
        // Content should be equal
        assert_eq!(&*s1, "test");
        assert_eq!(&*s2, "test");
        assert_eq!(&*s3, "different");
    }
    
    #[test]
    fn test_global_interner() {
        let s1 = intern_pattern("global_test");
        let s2 = intern_pattern("global_test");
        
        assert!(Arc::ptr_eq(&s1, &s2));
        assert_eq!(&*s1, "global_test");
    }
    
    #[test]
    fn test_interner_stats() {
        let interner = StringInterner::new();
        
        // Add some strings
        let _s1 = interner.intern("stat_test_1");
        let _s2 = interner.intern("stat_test_2");
        let _s3 = interner.intern("stat_test_1"); // Duplicate
        
        let stats = interner.stats();
        assert_eq!(stats.unique_strings, 2);
        assert_eq!(stats.poison_events, 0);
    }
    
    #[test]
    fn test_capacity_eviction() {
        // Test that entries are evicted when capacity is reached
        let config = StringInternerConfig {
            max_capacity: 10, // Small capacity for testing
        };
        let interner = StringInterner::with_config(config);
        
        // Fill up to capacity
        for i in 0..10 {
            interner.intern(&format!("string_{}", i));
        }
        
        let stats = interner.stats();
        assert_eq!(stats.unique_strings, 10);
        
        // Add more strings to trigger eviction
        for i in 10..15 {
            interner.intern(&format!("string_{}", i));
        }
        
        // After eviction, should have fewer entries than total added
        let stats = interner.stats();
        assert!(stats.unique_strings <= 10);
        assert!(stats.unique_strings >= 5); // At least half should remain
    }
    
    #[test]
    fn test_custom_capacity() {
        let config = StringInternerConfig {
            max_capacity: 100,
        };
        let interner = StringInterner::with_config(config);
        
        // Add many strings
        for i in 0..50 {
            interner.intern(&format!("test_{}", i));
        }
        
        let stats = interner.stats();
        assert_eq!(stats.unique_strings, 50);
    }
}
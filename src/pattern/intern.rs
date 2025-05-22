//! String interning for pattern matching to reduce allocations
//!
//! This module provides a thread-safe string interner that allows multiple
//! patterns to share the same string storage, reducing memory overhead
//! and improving cache locality.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;

/// Global string interner for pattern strings
static PATTERN_INTERNER: Lazy<StringInterner> = Lazy::new(StringInterner::new);

/// Thread-safe string interner
pub struct StringInterner {
    strings: RwLock<HashMap<String, Arc<str>>>,
}

impl StringInterner {
    /// Create a new string interner
    pub fn new() -> Self {
        Self {
            strings: RwLock::new(HashMap::new()),
        }
    }
    
    /// Intern a string, returning a shared reference
    pub fn intern(&self, s: &str) -> Arc<str> {
        // First try to get from read lock (fast path)
        if let Ok(strings) = self.strings.read() {
            if let Some(interned) = strings.get(s) {
                return Arc::clone(interned);
            }
        }
        
        // Need to insert, acquire write lock with poison recovery
        let mut strings = match self.strings.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("StringInterner write lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        
        // Check again in case another thread inserted while we waited
        if let Some(interned) = strings.get(s) {
            return Arc::clone(interned);
        }
        
        // Insert new string
        let interned: Arc<str> = Arc::from(s);
        strings.insert(s.to_string(), Arc::clone(&interned));
        interned
    }
    
    /// Get statistics about the interner
    pub fn stats(&self) -> InternerStats {
        let strings = match self.strings.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("StringInterner read lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        InternerStats {
            unique_strings: strings.len(),
            estimated_memory_saved: strings.iter()
                .map(|(k, _)| k.len() * (Arc::strong_count(&strings[k]).saturating_sub(1)))
                .sum(),
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
        // Should have some memory savings from the duplicate
        assert!(stats.estimated_memory_saved > 0);
    }
    
    #[test]
    fn test_poison_recovery() {
        use std::sync::Arc;
        use std::thread;
        
        let interner = Arc::new(StringInterner::new());
        
        // Simulate poison by panicking while holding the lock
        let interner_clone = Arc::clone(&interner);
        let handle = thread::spawn(move || {
            let _guard = interner_clone.strings.write().unwrap();
            panic!("Simulating poison");
        });
        
        // Wait for the thread to panic and poison the lock
        let _ = handle.join();
        
        // The interner should still work despite the poisoned lock
        let result = interner.intern("poison_test");
        assert_eq!(&*result, "poison_test");
        
        // Stats should also work with poisoned lock
        let stats = interner.stats();
        assert_eq!(stats.unique_strings, 1);
    }
}
//! Numeric pattern matching implementations

use crate::pattern::traits::NumMatcher;

/// Pattern for exact numeric matching
#[derive(Debug, Clone)]
pub struct NumPattern {
    /// The numeric value to match
    pub value: i64,
}

impl NumMatcher for NumPattern {
    fn num_match(&self, value: i64) -> bool {
        self.value == value
    }
}

/// Collection of numeric matchers (OR logic)
#[derive(Debug)]
pub struct NumMatchers {
    matchers: Vec<Box<dyn NumMatcher>>,
}

impl NumMatchers {
    /// Create a new collection of numeric matchers
    pub fn new(matchers: Vec<Box<dyn NumMatcher>>) -> Self {
        Self { matchers }
    }
}

impl NumMatcher for NumMatchers {
    fn num_match(&self, value: i64) -> bool {
        self.matchers.iter().any(|m| m.num_match(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_pattern() {
        let pattern = NumPattern { value: 42 };
        
        assert!(pattern.num_match(42));
        assert!(!pattern.num_match(41));
        assert!(!pattern.num_match(43));
    }

    #[test]
    fn test_num_matchers() {
        let matchers: Vec<Box<dyn NumMatcher>> = vec![
            Box::new(NumPattern { value: 1 }),
            Box::new(NumPattern { value: 2 }),
            Box::new(NumPattern { value: 3 }),
        ];

        let collection = NumMatchers::new(matchers);
        
        assert!(collection.num_match(1));
        assert!(collection.num_match(2));
        assert!(collection.num_match(3));
        assert!(!collection.num_match(4));
    }
}
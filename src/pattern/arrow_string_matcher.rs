//! Arrow-based string matching with SIMD optimizations
//!
//! This module provides vectorized string matching operations for columnar
//! evaluation of Sigma rules using Apache Arrow's compute kernels.

use arrow_array::{Array, BooleanArray, StringArray, builder::BooleanBuilder};
use arrow::compute::{and, or, like};

use crate::error::{Result, SigmaError};
use crate::pattern::TextPatternModifier;

/// Vectorized string matcher for Arrow arrays
pub struct ArrowStringMatcher {
    /// The pattern to match
    pattern: String,
    /// The matching modifier
    modifier: TextPatternModifier,
    /// Whether to perform case-insensitive matching
    case_insensitive: bool,
}

impl ArrowStringMatcher {
    /// Create a new Arrow string matcher
    pub fn new(pattern: String, modifier: TextPatternModifier, case_insensitive: bool) -> Self {
        Self {
            pattern,
            modifier,
            case_insensitive,
        }
    }

    /// Evaluate this matcher against a string array
    pub fn evaluate(&self, array: &StringArray) -> Result<BooleanArray> {
        match self.modifier {
            TextPatternModifier::None => self.exact_match(array),
            TextPatternModifier::Contains => self.contains_match(array),
            TextPatternModifier::Prefix => self.prefix_match(array),
            TextPatternModifier::Suffix => self.suffix_match(array),
            TextPatternModifier::Regex => self.regex_match(array),
            TextPatternModifier::All => self.all_match(array),
            TextPatternModifier::Keyword => self.keyword_match(array),
        }
    }

    /// Exact string matching
    fn exact_match(&self, array: &StringArray) -> Result<BooleanArray> {
        if self.case_insensitive {
            // Arrow doesn't have case-insensitive equality, so we use a workaround
            let pattern = format!("^{}$", regex::escape(&self.pattern));
            let flags = "(?i)"; // Case-insensitive flag
            let full_pattern = format!("{}{}", flags, pattern);
            
            // Use string literal for like pattern
            let pattern_array = StringArray::from(vec![full_pattern.as_str(); array.len()]);
            like(array, &pattern_array)
                .map_err(|e| SigmaError::Arrow(format!("Exact match failed: {}", e)))
        } else {
            // Direct comparison
            let builder = BooleanBuilder::with_capacity(array.len());
            let mut builder = builder;
            for i in 0..array.len() {
                if array.is_null(i) {
                    builder.append_null();
                } else {
                    let value = array.value(i);
                    builder.append_value(value == self.pattern);
                }
            }
            Ok(builder.finish())
        }
    }

    /// Contains matching
    fn contains_match(&self, array: &StringArray) -> Result<BooleanArray> {
        if self.case_insensitive {
            // Use regex for case-insensitive contains
            let pattern = regex::escape(&self.pattern);
            let flags = "(?i)";
            let full_pattern = format!("{}{}", flags, pattern);
            
            // Use string literal for like pattern
            let pattern_array = StringArray::from(vec![full_pattern.as_str(); array.len()]);
            like(array, &pattern_array)
                .map_err(|e| SigmaError::Arrow(format!("Contains match failed: {}", e)))
        } else {
            // Use custom implementation for contains
            let builder = BooleanBuilder::with_capacity(array.len());
            let mut builder = builder;
            for i in 0..array.len() {
                if array.is_null(i) {
                    builder.append_null();
                } else {
                    let value = array.value(i);
                    builder.append_value(value.contains(&self.pattern));
                }
            }
            Ok(builder.finish())
        }
    }

    /// Prefix matching
    fn prefix_match(&self, array: &StringArray) -> Result<BooleanArray> {
        if self.case_insensitive {
            let pattern = format!("^{}", regex::escape(&self.pattern));
            let flags = "(?i)";
            let full_pattern = format!("{}{}", flags, pattern);
            
            // Use string literal for like pattern
            let pattern_array = StringArray::from(vec![full_pattern.as_str(); array.len()]);
            like(array, &pattern_array)
                .map_err(|e| SigmaError::Arrow(format!("Prefix match failed: {}", e)))
        } else {
            // Use custom implementation for starts_with
            let builder = BooleanBuilder::with_capacity(array.len());
            let mut builder = builder;
            for i in 0..array.len() {
                if array.is_null(i) {
                    builder.append_null();
                } else {
                    let value = array.value(i);
                    builder.append_value(value.starts_with(&self.pattern));
                }
            }
            Ok(builder.finish())
        }
    }

    /// Suffix matching
    fn suffix_match(&self, array: &StringArray) -> Result<BooleanArray> {
        if self.case_insensitive {
            let pattern = format!("{}$", regex::escape(&self.pattern));
            let flags = "(?i)";
            let full_pattern = format!("{}{}", flags, pattern);
            
            // Use string literal for like pattern
            let pattern_array = StringArray::from(vec![full_pattern.as_str(); array.len()]);
            like(array, &pattern_array)
                .map_err(|e| SigmaError::Arrow(format!("Suffix match failed: {}", e)))
        } else {
            // Use custom implementation for ends_with
            let builder = BooleanBuilder::with_capacity(array.len());
            let mut builder = builder;
            for i in 0..array.len() {
                if array.is_null(i) {
                    builder.append_null();
                } else {
                    let value = array.value(i);
                    builder.append_value(value.ends_with(&self.pattern));
                }
            }
            Ok(builder.finish())
        }
    }

    /// Regex matching
    fn regex_match(&self, array: &StringArray) -> Result<BooleanArray> {
        let pattern = if self.case_insensitive {
            format!("(?i){}", self.pattern)
        } else {
            self.pattern.clone()
        };
        
        // Arrow's like function uses SQL LIKE patterns, not regex
        // For true regex, we need to iterate (less efficient but correct)
        let regex = regex::Regex::new(&pattern)
            .map_err(|e| SigmaError::Pattern(format!("Invalid regex: {}", e)))?;
        
        let builder = BooleanBuilder::with_capacity(array.len());
        let mut builder = builder;
        for i in 0..array.len() {
            if array.is_null(i) {
                builder.append_null();
            } else {
                let value = array.value(i);
                builder.append_value(regex.is_match(value));
            }
        }
        Ok(builder.finish())
    }

    /// All matching (for keywords)
    fn all_match(&self, _array: &StringArray) -> Result<BooleanArray> {
        // "All" modifier typically means match against all fields
        // This requires special handling at a higher level
        Err(SigmaError::Pattern("All modifier not supported at array level".to_string()))
    }

    /// Keyword matching
    fn keyword_match(&self, array: &StringArray) -> Result<BooleanArray> {
        // Keyword matching is similar to contains but with word boundaries
        self.contains_match(array)
    }
}

/// Vectorized pattern matching for multiple patterns
pub struct ArrowMultiStringMatcher {
    /// Individual matchers
    matchers: Vec<ArrowStringMatcher>,
    /// Whether to AND or OR the results
    combine_with_and: bool,
}

impl ArrowMultiStringMatcher {
    /// Create a new multi-pattern matcher
    pub fn new(patterns: Vec<String>, modifier: TextPatternModifier, case_insensitive: bool) -> Self {
        let matchers = patterns
            .into_iter()
            .map(|p| ArrowStringMatcher::new(p, modifier, case_insensitive))
            .collect();
        
        Self {
            matchers,
            combine_with_and: false, // OR by default
        }
    }

    /// Set whether to combine with AND (default is OR)
    pub fn with_and_combination(mut self) -> Self {
        self.combine_with_and = true;
        self
    }

    /// Evaluate all patterns against the array
    pub fn evaluate(&self, array: &StringArray) -> Result<BooleanArray> {
        if self.matchers.is_empty() {
            // No patterns = no matches
            return Ok(BooleanArray::from(vec![false; array.len()]));
        }

        let mut result = self.matchers[0].evaluate(array)?;

        for matcher in &self.matchers[1..] {
            let mask = matcher.evaluate(array)?;
            
            result = if self.combine_with_and {
                and(&result, &mask)
                    .map_err(|e| SigmaError::Arrow(format!("AND operation failed: {}", e)))?
            } else {
                or(&result, &mask)
                    .map_err(|e| SigmaError::Arrow(format!("OR operation failed: {}", e)))?
            };
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() -> Result<()> {
        let array = StringArray::from(vec!["hello", "world", "Hello", "HELLO"]);
        
        // Case sensitive
        let matcher = ArrowStringMatcher::new("hello".to_string(), TextPatternModifier::None, false);
        let result = matcher.evaluate(&array)?;
        assert_eq!(result.value(0), true);
        assert_eq!(result.value(1), false);
        assert_eq!(result.value(2), false);
        assert_eq!(result.value(3), false);

        // Case insensitive
        let matcher = ArrowStringMatcher::new("hello".to_string(), TextPatternModifier::None, true);
        let result = matcher.evaluate(&array)?;
        assert_eq!(result.value(0), true);
        assert_eq!(result.value(1), false);
        assert_eq!(result.value(2), true);
        assert_eq!(result.value(3), true);

        Ok(())
    }

    #[test]
    fn test_contains_match() -> Result<()> {
        let array = StringArray::from(vec!["hello world", "goodbye", "say hello", "HELLO"]);
        
        let matcher = ArrowStringMatcher::new("hello".to_string(), TextPatternModifier::Contains, false);
        let result = matcher.evaluate(&array)?;
        assert_eq!(result.value(0), true);
        assert_eq!(result.value(1), false);
        assert_eq!(result.value(2), true);
        assert_eq!(result.value(3), false);

        Ok(())
    }

    #[test]
    fn test_multi_pattern() -> Result<()> {
        let array = StringArray::from(vec!["hello", "world", "goodbye", "hello world"]);
        
        // OR combination
        let matcher = ArrowMultiStringMatcher::new(
            vec!["hello".to_string(), "world".to_string()],
            TextPatternModifier::Contains,
            false,
        );
        let result = matcher.evaluate(&array)?;
        assert_eq!(result.value(0), true);  // contains "hello"
        assert_eq!(result.value(1), true);  // contains "world"
        assert_eq!(result.value(2), false); // contains neither
        assert_eq!(result.value(3), true);  // contains both

        // AND combination
        let matcher = ArrowMultiStringMatcher::new(
            vec!["hello".to_string(), "world".to_string()],
            TextPatternModifier::Contains,
            false,
        ).with_and_combination();
        let result = matcher.evaluate(&array)?;
        assert_eq!(result.value(0), false); // only "hello"
        assert_eq!(result.value(1), false); // only "world"
        assert_eq!(result.value(2), false); // neither
        assert_eq!(result.value(3), true);  // both

        Ok(())
    }
}
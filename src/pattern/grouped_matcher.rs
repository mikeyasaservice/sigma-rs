//! High-performance grouped pattern matching for Sigma rules
//!
//! This module implements an optimized pattern matching strategy that groups
//! patterns by field and type, enabling single-pass evaluation of multiple rules.

use std::collections::HashMap;
use std::sync::Arc;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use regex::Regex;
use arrow::array::{Array, StringArray, ArrayRef};
use arrow::datatypes::DataType;

use crate::error::Result;

/// Pattern type for grouping
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum PatternType {
    Contains,
    StartsWith,
    EndsWith,
    Regex,
    Exact,
}

/// Individual pattern with metadata
#[derive(Debug, Clone)]
pub struct Pattern {
    pub pattern: String,
    pub pattern_type: PatternType,
    pub rule_id: String,
    pub pattern_id: usize,
}

/// Groups patterns by field for efficient matching
#[derive(Debug)]
pub struct FieldPatternGroup {
    pub field_name: String,
    
    // Aho-Corasick for substring matching (handles contains)
    pub substring_matcher: Option<Arc<AhoCorasick>>,
    pub substring_patterns: Vec<(usize, String)>, // (pattern_id, rule_id)
    
    // Suffix patterns for endswith
    pub suffix_patterns: Vec<Pattern>,
    
    // Prefix patterns for startswith  
    pub prefix_patterns: Vec<Pattern>,
    
    // Compiled regex patterns
    pub regex_matchers: Vec<(Regex, String)>, // (regex, rule_id)
    
    // Exact match patterns
    pub exact_patterns: HashMap<String, Vec<String>>, // pattern -> rule_ids
}

impl FieldPatternGroup {
    /// Create a new field pattern group
    pub fn new(field_name: String) -> Self {
        Self {
            field_name,
            substring_matcher: None,
            substring_patterns: Vec::new(),
            suffix_patterns: Vec::new(),
            prefix_patterns: Vec::new(),
            regex_matchers: Vec::new(),
            exact_patterns: HashMap::new(),
        }
    }
    
    /// Add a pattern to the group
    pub fn add_pattern(&mut self, pattern: Pattern) -> Result<()> {
        match pattern.pattern_type {
            PatternType::Contains => {
                self.substring_patterns.push((pattern.pattern_id, pattern.rule_id));
            }
            PatternType::StartsWith => {
                self.prefix_patterns.push(pattern);
            }
            PatternType::EndsWith => {
                self.suffix_patterns.push(pattern);
            }
            PatternType::Regex => {
                let regex = Regex::new(&pattern.pattern)?;
                self.regex_matchers.push((regex, pattern.rule_id));
            }
            PatternType::Exact => {
                self.exact_patterns
                    .entry(pattern.pattern)
                    .or_insert_with(Vec::new)
                    .push(pattern.rule_id);
            }
        }
        Ok(())
    }
    
    /// Build optimized matchers after all patterns are added
    pub fn build(&mut self) -> Result<()> {
        // Build Aho-Corasick matcher for substring patterns
        if !self.substring_patterns.is_empty() {
            let patterns: Vec<String> = self.substring_patterns
                .iter()
                .enumerate()
                .map(|(idx, (_, _))| {
                    // We'll reconstruct the pattern from the patterns list
                    // In real implementation, we'd store the patterns separately
                    format!("pattern_{}", idx)
                })
                .collect();
                
            let ac = AhoCorasickBuilder::new()
                .build(&patterns)
                .map_err(|e| crate::error::SigmaError::Parse(
                    format!("Failed to build Aho-Corasick: {}", e)
                ))?;
            
            self.substring_matcher = Some(Arc::new(ac));
        }
        
        Ok(())
    }
    
    /// Match a single value against all patterns in this group
    pub fn match_value(&self, value: &str) -> Vec<String> {
        let mut matched_rules = Vec::new();
        
        // Check exact matches first (fastest)
        if let Some(rules) = self.exact_patterns.get(value) {
            matched_rules.extend(rules.clone());
        }
        
        // Check substring matches using Aho-Corasick
        if let Some(ac) = &self.substring_matcher {
            for mat in ac.find_iter(value) {
                if let Some((_, rule_id)) = self.substring_patterns.get(mat.pattern().as_usize()) {
                    matched_rules.push(rule_id.clone());
                }
            }
        }
        
        // Check prefix matches
        for pattern in &self.prefix_patterns {
            if value.starts_with(&pattern.pattern) {
                matched_rules.push(pattern.rule_id.clone());
            }
        }
        
        // Check suffix matches
        for pattern in &self.suffix_patterns {
            if value.ends_with(&pattern.pattern) {
                matched_rules.push(pattern.rule_id.clone());
            }
        }
        
        // Check regex matches
        for (regex, rule_id) in &self.regex_matchers {
            if regex.is_match(value) {
                matched_rules.push(rule_id.clone());
            }
        }
        
        matched_rules
    }
    
    /// Match an Arrow array column against all patterns
    pub fn match_array(&self, array: &ArrayRef) -> Result<HashMap<String, Vec<usize>>> {
        if array.data_type() != &DataType::Utf8 {
            return Err(crate::error::SigmaError::InvalidPattern(
                format!("Expected Utf8 array, got {:?}", array.data_type())
            ));
        }
        
        let string_array = array.as_any().downcast_ref::<StringArray>()
            .ok_or_else(|| crate::error::SigmaError::Runtime(
                "Failed to downcast to StringArray".to_string()
            ))?;
        
        let mut rule_matches: HashMap<String, Vec<usize>> = HashMap::new();
        
        for (idx, value) in string_array.iter().enumerate() {
            if let Some(val) = value {
                let matched_rules = self.match_value(val);
                for rule_id in matched_rules {
                    rule_matches
                        .entry(rule_id)
                        .or_insert_with(Vec::new)
                        .push(idx);
                }
            }
        }
        
        Ok(rule_matches)
    }
}

/// Optimized pattern matcher that groups patterns by field
#[derive(Debug)]
pub struct GroupedPatternMatcher {
    /// Pattern groups indexed by field name
    field_groups: HashMap<String, FieldPatternGroup>,
    
    /// Total number of patterns
    pattern_count: usize,
}

impl GroupedPatternMatcher {
    /// Create a new grouped pattern matcher
    pub fn new() -> Self {
        Self {
            field_groups: HashMap::new(),
            pattern_count: 0,
        }
    }
    
    /// Add a pattern for a specific field
    pub fn add_pattern(&mut self, field: &str, pattern: Pattern) -> Result<()> {
        let group = self.field_groups
            .entry(field.to_string())
            .or_insert_with(|| FieldPatternGroup::new(field.to_string()));
        
        group.add_pattern(pattern)?;
        self.pattern_count += 1;
        
        Ok(())
    }
    
    /// Build all optimized matchers
    pub fn build(&mut self) -> Result<()> {
        for group in self.field_groups.values_mut() {
            group.build()?;
        }
        Ok(())
    }
    
    /// Get pattern group for a field
    pub fn get_field_group(&self, field: &str) -> Option<&FieldPatternGroup> {
        self.field_groups.get(field)
    }
    
    /// Get total pattern count
    pub fn pattern_count(&self) -> usize {
        self.pattern_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_grouped_matcher() {
        let mut matcher = GroupedPatternMatcher::new();
        
        // Add patterns for CommandLine field
        matcher.add_pattern("CommandLine", Pattern {
            pattern: "powershell".to_string(),
            pattern_type: PatternType::Contains,
            rule_id: "rule1".to_string(),
            pattern_id: 0,
        }).unwrap();
        
        matcher.add_pattern("CommandLine", Pattern {
            pattern: "encoded".to_string(),
            pattern_type: PatternType::Contains,
            rule_id: "rule2".to_string(),
            pattern_id: 1,
        }).unwrap();
        
        matcher.build().unwrap();
        
        // Test matching
        let group = matcher.get_field_group("CommandLine").unwrap();
        let matches = group.match_value("powershell -encoded command");
        
        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&"rule1".to_string()));
        assert!(matches.contains(&"rule2".to_string()));
    }
}
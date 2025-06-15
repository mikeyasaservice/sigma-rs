//! Tiered rule compiler for optimized evaluation
//!
//! This compiler categorizes rules into three tiers:
//! - Tier 1: Simple field equality (fastest)
//! - Tier 2: String pattern matching (90% of rules)
//! - Tier 3: Complex expressions requiring DataFusion

use std::collections::HashMap;
use std::sync::Arc;
use datafusion::logical_expr::Expr;
use serde::{Serialize, Deserialize};

use crate::error::Result;
use crate::pattern::grouped_matcher::{Pattern, PatternType, GroupedPatternMatcher};
use crate::rule::Rule;

/// Temporary AST representation until full parser is integrated
#[derive(Debug, Clone)]
enum ParsedCondition {
    Simple {
        field: String,
        value: serde_json::Value,
    },
    Pattern {
        field: String,
        pattern: String,
        pattern_type: PatternType,
    },
    Complex {
        expr: String,
    },
}

/// Compiled rule with tier assignment
#[derive(Debug, Clone)]
pub enum TieredRule {
    /// Simple field equality check
    Simple {
        rule_id: String,
        field: String,
        value: serde_json::Value,
    },
    
    /// Pattern-based matching (majority of rules)
    Pattern {
        rule_id: String,
        required_fields: Vec<String>,
        pattern_refs: Vec<PatternRef>,
        logic: BooleanExpression,
    },
    
    /// Complex expression requiring DataFusion
    Complex {
        rule_id: String,
        datafusion_expr: Expr,
        estimated_cost: usize,
    },
}

/// Reference to a pattern in the grouped matcher
#[derive(Debug, Clone)]
pub struct PatternRef {
    pub field: String,
    pub pattern_id: usize,
    pub negate: bool,
}

/// Boolean expression for combining pattern results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BooleanExpression {
    And(Vec<BooleanExpression>),
    Or(Vec<BooleanExpression>),
    Not(Box<BooleanExpression>),
    PatternRef(usize), // Index into pattern_refs
}

/// Statistics about rule compilation
#[derive(Debug, Default)]
pub struct CompilationStats {
    pub total_rules: usize,
    pub simple_rules: usize,
    pub pattern_rules: usize,
    pub complex_rules: usize,
    pub total_patterns: usize,
    pub patterns_by_type: HashMap<PatternType, usize>,
}

/// Tiered rule compiler
pub struct TieredCompiler {
    /// Grouped pattern matcher
    pattern_matcher: Arc<GroupedPatternMatcher>,
    
    /// Compiled rules by tier
    simple_rules: Vec<TieredRule>,
    pattern_rules: Vec<TieredRule>,
    complex_rules: Vec<TieredRule>,
    
    /// Compilation statistics
    stats: CompilationStats,
    
    /// Pattern ID counter
    next_pattern_id: usize,
}

impl TieredCompiler {
    /// Create a new tiered compiler
    pub fn new() -> Self {
        Self {
            pattern_matcher: Arc::new(GroupedPatternMatcher::new()),
            simple_rules: Vec::new(),
            pattern_rules: Vec::new(),
            complex_rules: Vec::new(),
            stats: CompilationStats::default(),
            next_pattern_id: 0,
        }
    }
    
    /// Compile a rule into the appropriate tier
    pub fn compile_rule(&mut self, rule: &Rule) -> Result<()> {
        self.stats.total_rules += 1;
        
        // Parse the condition
        // Parse detection condition
        let condition_str = rule.detection.condition().unwrap_or("selection");
        let ast = self.parse_condition(condition_str)?;
        
        // Analyze and categorize the rule
        match self.analyze_ast(&ast) {
            RuleCategory::Simple { field, value } => {
                self.stats.simple_rules += 1;
                self.simple_rules.push(TieredRule::Simple {
                    rule_id: rule.id.clone(),
                    field,
                    value,
                });
            }
            
            RuleCategory::Pattern { patterns, logic } => {
                self.stats.pattern_rules += 1;
                
                // Extract patterns and add to matcher
                let mut pattern_refs = Vec::new();
                let mut required_fields = Vec::new();
                
                for (field, pattern_str, pattern_type, negate) in patterns {
                    let pattern = Pattern {
                        pattern: pattern_str,
                        pattern_type: pattern_type.clone(),
                        rule_id: rule.id.clone(),
                        pattern_id: self.next_pattern_id,
                    };
                    
                    // Add to grouped matcher
                    Arc::get_mut(&mut self.pattern_matcher)
                        .unwrap()
                        .add_pattern(&field, pattern)?;
                    
                    pattern_refs.push(PatternRef {
                        field: field.clone(),
                        pattern_id: self.next_pattern_id,
                        negate,
                    });
                    
                    if !required_fields.contains(&field) {
                        required_fields.push(field);
                    }
                    
                    self.next_pattern_id += 1;
                    self.stats.total_patterns += 1;
                    *self.stats.patterns_by_type.entry(pattern_type).or_insert(0) += 1;
                }
                
                self.pattern_rules.push(TieredRule::Pattern {
                    rule_id: rule.id.clone(),
                    required_fields,
                    pattern_refs,
                    logic,
                });
            }
            
            RuleCategory::Complex => {
                self.stats.complex_rules += 1;
                
                // Fall back to DataFusion compilation
                let expr = self.compile_to_datafusion(&ast)?;
                let cost = self.estimate_cost(&expr);
                
                self.complex_rules.push(TieredRule::Complex {
                    rule_id: rule.id.clone(),
                    datafusion_expr: expr,
                    estimated_cost: cost,
                });
            }
        }
        
        Ok(())
    }
    
    /// Build optimized matchers after all rules are compiled
    pub fn build(&mut self) -> Result<()> {
        Arc::get_mut(&mut self.pattern_matcher)
            .unwrap()
            .build()?;
        Ok(())
    }
    
    /// Get compilation statistics
    pub fn stats(&self) -> &CompilationStats {
        &self.stats
    }
    
    /// Get rules by tier
    pub fn get_simple_rules(&self) -> &[TieredRule] {
        &self.simple_rules
    }
    
    pub fn get_pattern_rules(&self) -> &[TieredRule] {
        &self.pattern_rules
    }
    
    pub fn get_complex_rules(&self) -> &[TieredRule] {
        &self.complex_rules
    }
    
    pub fn get_pattern_matcher(&self) -> Arc<GroupedPatternMatcher> {
        Arc::clone(&self.pattern_matcher)
    }
    
    // Helper methods
    
    fn parse_condition(&self, _condition: &str) -> Result<ParsedCondition> {
        // TODO: Use actual parser
        // For now, return a placeholder
        Ok(ParsedCondition::Simple {
            field: "placeholder".to_string(),
            value: serde_json::Value::String("value".to_string()),
        })
    }
    
    fn analyze_ast(&self, ast: &ParsedCondition) -> RuleCategory {
        // Analyze AST to determine rule category
        match ast {
            ParsedCondition::Simple { field, value } => {
                return RuleCategory::Simple {
                    field: field.clone(),
                    value: value.clone(),
                };
            }
            _ => {}
        }
        
        // For now, default to complex
        // TODO: Implement proper pattern extraction
        RuleCategory::Complex
    }
    
    fn compile_to_datafusion(&self, _ast: &ParsedCondition) -> Result<Expr> {
        // TODO: Implement DataFusion compilation
        Ok(Expr::Literal(datafusion::scalar::ScalarValue::Boolean(Some(true))))
    }
    
    fn estimate_cost(&self, _expr: &Expr) -> usize {
        // TODO: Implement cost estimation
        100
    }
}

/// Rule categorization result
enum RuleCategory {
    Simple {
        field: String,
        value: serde_json::Value,
    },
    Pattern {
        patterns: Vec<(String, String, PatternType, bool)>, // (field, pattern, type, negate)
        logic: BooleanExpression,
    },
    Complex,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::Detection;
    
    #[test]
    fn test_tiered_compilation() {
        let mut compiler = TieredCompiler::new();
        
        // Create a simple test rule
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::json!("selection"));
        
        let rule = Rule {
            id: "test_rule".to_string(),
            title: "Test Rule".to_string(),
            description: Some("Test rule".to_string()),
            level: Some("medium".to_string()),
            detection,
            tags: vec![],
            logsource: Default::default(),
            author: None,
            falsepositives: vec![],
            fields: vec![],
            status: None,
            references: vec![],
            date: None,
            modified: None,
        };
        
        compiler.compile_rule(&rule).unwrap();
        compiler.build().unwrap();
        
        let stats = compiler.stats();
        assert_eq!(stats.total_rules, 1);
    }
}
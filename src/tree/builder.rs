use std::sync::Arc;
use globset::GlobBuilder;

use crate::ast::{Branch, nodes::{NodeAnd, NodeOr, NodeNot, NodeSimpleAnd, NodeSimpleOr, Identifier}};
use crate::pattern::IdentifierType;
use crate::lexer::{Token, Item};
use crate::parser::{Parser, ParseError};
use crate::rule::{Detection, RuleHandle};
use crate::tree::Tree;

/// Maximum recursion depth for nested expressions to prevent stack overflow
const MAX_RECURSION_DEPTH: usize = 50;

/// Maximum number of wildcards allowed in a single glob pattern
const MAX_GLOB_WILDCARDS: usize = 10;

/// Initial capacity for branch vectors
const INITIAL_AND_CAPACITY: usize = 8;
const INITIAL_OR_CAPACITY: usize = 4;

/// Maximum number of branches to prevent memory exhaustion
const MAX_BRANCHES: usize = 1000;

/// Build a new Tree from a RuleHandle
pub async fn build_tree(rule: RuleHandle) -> Result<Tree, ParseError> {
    let _condition = rule.rule.detection.condition()
        .ok_or_else(|| ParseError::MissingCondition)?;
    
    // Create parser with detection
    let mut parser = Parser::new(rule.rule.detection.clone(), rule.no_collapse_ws);
    
    // Run parser with rule context for better error messages
    parser.run().await.map_err(|e| {
        // Enhance error with rule context where possible
        match e {
            ParseError::ParserError(msg) => ParseError::detection_parsing_failed(&rule.rule.id, msg),
            ParseError::NoValidFieldPatterns { field, errors, .. } => {
                ParseError::no_valid_field_patterns(&rule.rule.id, field, errors)
            }
            ParseError::FieldPatternCreationFailed { field, value, error } => {
                ParseError::detection_parsing_failed(
                    &rule.rule.id, 
                    format!("Failed to create pattern for field '{}' with value '{}': {}", field, value, error)
                )
            }
            ParseError::InvalidGlobPattern { pattern, error } => {
                ParseError::detection_parsing_failed(
                    &rule.rule.id,
                    format!("Invalid glob pattern '{}': {}", pattern, error)
                )
            }
            ParseError::RecursionLimitExceeded { current, limit } => {
                ParseError::detection_parsing_failed(
                    &rule.rule.id,
                    format!("Recursion limit exceeded: {} (limit: {})", current, limit)
                )
            }
            ParseError::InvalidBranchStructure { message } => {
                ParseError::detection_parsing_failed(
                    &rule.rule.id,
                    format!("Invalid branch structure: {}", message)
                )
            }
            ParseError::TokenLimitExceeded { current, limit } => {
                ParseError::detection_parsing_failed(
                    &rule.rule.id,
                    format!("Token limit exceeded: {} (limit: {})", current, limit)
                )
            }
            ParseError::MemoryLimitExceeded { current_bytes, limit_bytes } => {
                ParseError::detection_parsing_failed(
                    &rule.rule.id,
                    format!("Memory limit exceeded: {} bytes (limit: {} bytes)", current_bytes, limit_bytes)
                )
            }
            _ => ParseError::detection_parsing_failed(&rule.rule.id, e.to_string()),
        }
    })?;
    
    // Get the root AST node
    let root = parser.result()
        .ok_or_else(|| ParseError::MissingCondition)?;
    
    // Create tree with root and rule
    Ok(Tree::new(root, Arc::new(rule)))
}

/// Build a branch from token sequence
pub fn build_branch(
    detection: &Detection,
    tokens: Vec<Item>,
    depth: usize,
    no_collapse_ws: bool,
) -> Result<Arc<dyn Branch>, ParseError> {
    // Check recursion depth to prevent stack overflow
    if depth > MAX_RECURSION_DEPTH {
        return Err(ParseError::RecursionLimitExceeded {
            current: depth,
            limit: MAX_RECURSION_DEPTH,
        });
    }
    
    let mut iter = tokens.into_iter().peekable();
    
    let mut and_nodes: Vec<Arc<dyn Branch>> = Vec::with_capacity(INITIAL_AND_CAPACITY);
    let mut or_nodes: Vec<Arc<dyn Branch>> = Vec::with_capacity(INITIAL_OR_CAPACITY);
    let mut negated = false;
    let mut wildcard = None;
    
    while let Some(item) = iter.next() {
        match item.token {
            Token::Identifier => {
                let value = detection.get(&item.value)
                    .ok_or_else(|| ParseError::MissingConditionItem { 
                        key: item.value.clone() 
                    })?;
                
                let ident_type = identify_type(&item.value, value);
                let branch = build_rule_from_ident(value, ident_type, no_collapse_ws)?;
                
                and_nodes.push(if negated {
                    Arc::new(NodeNot::new(branch))
                } else {
                    branch
                });
                negated = false;
            }
            
            Token::KeywordAnd => {
                // Continue building AND chain
            }
            
            Token::KeywordOr => {
                // Collect current AND nodes into OR
                let and_branch = reduce_branches(and_nodes, BranchType::And)?;
                or_nodes.push(and_branch);
                and_nodes = Vec::with_capacity(INITIAL_AND_CAPACITY);
            }
            
            Token::KeywordNot => {
                negated = true;
            }
            
            Token::SepLpar => {
                // Extract group and build recursively
                let group_items = extract_group_items(&mut iter)?;
                let branch = build_branch(detection, group_items, depth + 1, no_collapse_ws)?;
                
                and_nodes.push(if negated {
                    Arc::new(NodeNot::new(branch))
                } else {
                    branch
                });
                negated = false;
            }
            
            Token::IdentifierAll => {
                let rules = match wildcard {
                    Some(Token::StmtAllOf) => extract_all_to_rules(detection, no_collapse_ws)?,
                    Some(Token::StmtOneOf) => extract_all_to_rules(detection, no_collapse_ws)?,
                    _ => return Err(ParseError::InvalidWildcardIdent),
                };
                
                let branch = if matches!(wildcard, Some(Token::StmtAllOf)) {
                    reduce_branches(rules, BranchType::And)?
                } else {
                    reduce_branches(rules, BranchType::Or)?
                };
                
                and_nodes.push(if negated {
                    Arc::new(NodeNot::new(branch))
                } else {
                    branch
                });
                negated = false;
            }
            
            Token::IdentifierWithWildcard => {
                // Validate glob pattern complexity
                validate_glob_pattern(&item.value)?;
                
                let glob = GlobBuilder::new(&item.value)
                    .literal_separator(false)
                    .build()
                    .map_err(|e| ParseError::InvalidGlobPattern { 
                        pattern: item.value.clone(), 
                        error: e.to_string() 
                    })?
                    .compile_matcher();
                
                let rules = match wildcard {
                    Some(Token::StmtAllOf) => {
                        extract_wildcard_idents(detection, &glob, no_collapse_ws)?
                    }
                    Some(Token::StmtOneOf) => {
                        extract_wildcard_idents(detection, &glob, no_collapse_ws)?
                    }
                    _ => return Err(ParseError::InvalidWildcardIdent),
                };
                
                let branch = if matches!(wildcard, Some(Token::StmtAllOf)) {
                    reduce_branches(rules, BranchType::And)?
                } else {
                    reduce_branches(rules, BranchType::Or)?
                };
                
                and_nodes.push(if negated {
                    Arc::new(NodeNot::new(branch))
                } else {
                    branch
                });
                negated = false;
                wildcard = None;
            }
            
            Token::StmtAllOf => {
                wildcard = Some(Token::StmtAllOf);
            }
            
            Token::StmtOneOf => {
                wildcard = Some(Token::StmtOneOf);
            }
            
            Token::SepRpar => {
                return Err(ParseError::UnexpectedToken { 
                    token: Token::SepRpar 
                });
            }
            
            _ => {
                return Err(ParseError::UnsupportedToken { 
                    msg: format!("{:?}", item.token) 
                });
            }
        }
    }
    
    // Check branch limits
    if and_nodes.len() + or_nodes.len() > MAX_BRANCHES {
        return Err(ParseError::ParserError(
            format!("Too many branches: {} (limit: {})", and_nodes.len() + or_nodes.len(), MAX_BRANCHES)
        ));
    }
    
    // Final reduction
    let and_branch = reduce_branches(and_nodes, BranchType::And)?;
    or_nodes.push(and_branch);
    
    if negated {
        match or_nodes.pop() {
            Some(node) => Ok(Arc::new(NodeNot::new(node))),
            None => Err(ParseError::ParserError(
                "No nodes available for negation".to_string()
            )),
        }
    } else {
        reduce_branches(or_nodes, BranchType::Or)
    }
}

enum BranchType {
    And,
    Or,
}

fn reduce_branches(mut branches: Vec<Arc<dyn Branch>>, branch_type: BranchType) -> Result<Arc<dyn Branch>, ParseError> {
    match branches.len() {
        0 => Err(ParseError::InvalidBranchStructure {
            message: "Cannot reduce empty branch list".to_string(),
        }),
        1 => branches.pop().ok_or_else(|| ParseError::InvalidBranchStructure {
            message: "Failed to pop from single-element branch list".to_string(),
        }),
        2 => {
            // More efficient handling of 2 elements using split_off
            let remaining = branches.split_off(1);
            let left = branches.into_iter().next().ok_or_else(|| ParseError::InvalidBranchStructure {
                message: "Failed to get left branch".to_string(),
            })?;
            let right = remaining.into_iter().next().ok_or_else(|| ParseError::InvalidBranchStructure {
                message: "Failed to get right branch".to_string(),
            })?;
            match branch_type {
                BranchType::And => Ok(Arc::new(NodeAnd::new(left, right))),
                BranchType::Or => Ok(Arc::new(NodeOr::new(left, right))),
            }
        }
        _ => {
            match branch_type {
                BranchType::And => Ok(Arc::new(NodeSimpleAnd::new(branches))),
                BranchType::Or => Ok(Arc::new(NodeSimpleOr::new(branches))),
            }
        }
    }
}

/// Extract group items without cloning to improve performance
fn extract_group_items(iter: &mut std::iter::Peekable<std::vec::IntoIter<Item>>) -> Result<Vec<Item>, ParseError> {
    let mut group = Vec::new();
    let mut balance = 1;
    
    while let Some(item) = iter.next() {
        match item.token {
            Token::SepLpar => {
                balance += 1;
                group.push(item);
            }
            Token::SepRpar => {
                balance -= 1;
                if balance == 0 {
                    // Don't include the closing paren
                    return Ok(group);
                } else {
                    group.push(item);
                }
            }
            _ => {
                group.push(item);
            }
        }
    }
    
    Err(ParseError::UnmatchedParenthesis)
}

/// Validate glob pattern complexity to prevent ReDoS
fn validate_glob_pattern(pattern: &str) -> Result<(), ParseError> {
    let wildcard_count = pattern.chars().filter(|&c| c == '*' || c == '?').count();
    
    if wildcard_count > MAX_GLOB_WILDCARDS {
        return Err(ParseError::InvalidGlobPattern {
            pattern: pattern.to_string(),
            error: format!("Too many wildcards: {} (limit: {})", wildcard_count, MAX_GLOB_WILDCARDS),
        });
    }
    
    // Check for pathological patterns
    if pattern.contains("**") || pattern.contains("*?*") || pattern.contains("?*?") {
        return Err(ParseError::InvalidGlobPattern {
            pattern: pattern.to_string(),
            error: "Pattern contains potentially dangerous wildcard sequences".to_string(),
        });
    }
    
    Ok(())
}

fn extract_all_to_rules(
    detection: &Detection,
    no_collapse_ws: bool,
) -> Result<Vec<Arc<dyn Branch>>, ParseError> {
    let mut rules = Vec::new();
    
    for (key, value) in detection.iter() {
        let ident_type = identify_type(key, value);
        let branch = build_rule_from_ident(value, ident_type, no_collapse_ws)?;
        rules.push(branch);
    }
    
    Ok(rules)
}

fn extract_wildcard_idents(
    detection: &Detection,
    glob: &globset::GlobMatcher,
    no_collapse_ws: bool,
) -> Result<Vec<Arc<dyn Branch>>, ParseError> {
    let rules: Result<Vec<_>, _> = detection
        .iter()
        .filter(|(key, _)| glob.is_match(key))
        .map(|(_, value)| build_rule_from_ident(value, IdentifierType::Selection, no_collapse_ws))
        .collect();
    
    let rules = rules?;
    
    if rules.is_empty() {
        return Err(ParseError::NoMatchingWildcard);
    }
    
    Ok(rules)
}

fn identify_type(_key: &str, value: &serde_json::Value) -> IdentifierType {
    match value {
        serde_json::Value::Array(_) => IdentifierType::Keywords,
        serde_json::Value::Object(_) => IdentifierType::Selection,
        _ => IdentifierType::Selection,
    }
}

fn build_rule_from_ident(
    value: &serde_json::Value,
    ident_type: IdentifierType,
    _no_collapse_ws: bool,
) -> Result<Arc<dyn Branch>, ParseError> {
    match ident_type {
        IdentifierType::Keywords => {
            // Handle keyword list
            if let Some(array) = value.as_array() {
                let keywords: Vec<String> = array
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                
                if !keywords.is_empty() {
                    // Create a keywords field pattern
                    let field_rule = crate::ast::FieldRule::new(
                        Arc::from("keywords"),
                        crate::ast::FieldPattern::Keywords(keywords),
                    );
                    return Ok(Arc::new(Identifier::from_rule(field_rule)));
                }
            }
            Err(ParseError::InvalidKeywordConstruct)
        }
        IdentifierType::Selection => {
            // Handle selection object
            if let Some(obj) = value.as_object() {
                tracing::error!("Processing selection object with {} fields", obj.len());
                // For single field selections, create a simple field rule
                if obj.len() == 1 {
                    let (key, val) = match obj.iter().next() {
                        Some((k, v)) => (k, v),
                        None => return Err(ParseError::ParserError(
                            "Empty object in selection".to_string()
                        )),
                    };
                    tracing::error!("Processing single field: key={}, val={:?}", key, val);
                    let (field_name, modifier) = parse_field_key(key);
                    tracing::error!("Parsed field: name={}, modifier={:?}", field_name, modifier);
                    let pattern = create_field_pattern_with_modifier(val, modifier)?;
                    let field_rule = crate::ast::FieldRule::new(Arc::from(field_name), pattern);
                    return Ok(Arc::new(Identifier::from_rule(field_rule)));
                }
                
                // For multiple fields, create an AND of field rules
                tracing::error!("Creating AND of multiple field rules");
                let branches: Vec<Arc<dyn Branch>> = obj
                    .iter()
                    .map(|(key, val)| {
                        tracing::error!("Processing field: key={}, val={:?}", key, val);
                        let (field_name, modifier) = parse_field_key(key);
                        tracing::error!("Parsed field: name={}, modifier={:?}", field_name, modifier);
                        let pattern = create_field_pattern_with_modifier(val, modifier)?;
                        let field_rule = crate::ast::FieldRule::new(Arc::from(field_name), pattern);
                        Ok(Arc::new(Identifier::from_rule(field_rule)) as Arc<dyn Branch>)
                    })
                    .collect::<Result<Vec<_>, ParseError>>()?;
                
                reduce_branches(branches, BranchType::And)
            } else {
                Err(ParseError::InvalidSelectionConstruct)
            }
        }
    }
}

fn parse_field_key(key: &str) -> (String, Option<crate::pattern::TextPatternModifier>) {
    if let Some(pos) = key.find('|') {
        let field = key[..pos].to_string();
        let modifier_str = &key[pos + 1..];
        let modifier = match modifier_str {
            "contains" => Some(crate::pattern::TextPatternModifier::Contains),
            "startswith" => Some(crate::pattern::TextPatternModifier::Prefix),
            "endswith" => Some(crate::pattern::TextPatternModifier::Suffix),
            "re" => Some(crate::pattern::TextPatternModifier::Regex),
            "all" => Some(crate::pattern::TextPatternModifier::All),
            _ => None,
        };
        (field, modifier)
    } else {
        (key.to_string(), None)
    }
}


fn create_field_pattern_with_modifier(value: &serde_json::Value, modifier: Option<crate::pattern::TextPatternModifier>) -> Result<crate::ast::FieldPattern, ParseError> {
    use crate::pattern::{new_string_matcher, new_num_matcher, TextPatternModifier};
    use std::sync::Arc;
    
    match value {
        serde_json::Value::String(s) => {
            // Use the modifier from field key if available, otherwise check for glob
            let modifier = modifier.unwrap_or_else(|| {
                if s.contains('*') || s.contains('?') {
                    TextPatternModifier::None  // Will be handled as glob by factory
                } else {
                    TextPatternModifier::None  // Exact match
                }
            });
            
            let matcher = new_string_matcher(
                modifier,
                false,  // lowercase
                false,  // all
                false,  // no_collapse_ws
                vec![s.clone()],
            ).map_err(|e| ParseError::UnsupportedValueType { 
                value_type: e 
            })?;
            
            Ok(crate::ast::FieldPattern::String {
                matcher: Arc::from(matcher),
                pattern_desc: Arc::from(s.clone()),
            })
        }
        serde_json::Value::Number(n) => {
            if let Some(num) = n.as_i64() {
                let matcher = new_num_matcher(vec![num])
                    .map_err(|e| ParseError::UnsupportedValueType { 
                        value_type: e 
                    })?;
                
                Ok(crate::ast::FieldPattern::Numeric {
                    matcher: Arc::from(matcher),
                    pattern_desc: Arc::from(n.to_string()),
                })
            } else {
                // Fall back to string matching for floats
                let matcher = new_string_matcher(
                    TextPatternModifier::None,
                    false,  // lowercase
                    false,  // all
                    false,  // no_collapse_ws
                    vec![n.to_string()],
                ).map_err(|e| ParseError::UnsupportedValueType { 
                    value_type: e 
                })?;
                
                Ok(crate::ast::FieldPattern::String {
                    matcher: Arc::from(matcher),
                    pattern_desc: Arc::from(n.to_string()),
                })
            }
        }
        serde_json::Value::Bool(b) => {
            let matcher = new_string_matcher(
                TextPatternModifier::None,
                false,  // lowercase
                false,  // all
                false,  // no_collapse_ws
                vec![b.to_string()],
            ).map_err(|e| ParseError::UnsupportedValueType { 
                value_type: e 
            })?;
            
            Ok(crate::ast::FieldPattern::String {
                matcher: Arc::from(matcher),
                pattern_desc: Arc::from(b.to_string()),
            })
        }
        serde_json::Value::Null => {
            let matcher = new_string_matcher(
                TextPatternModifier::None,
                false,  // lowercase
                false,  // all
                false,  // no_collapse_ws
                vec!["null".to_string()],
            ).map_err(|e| ParseError::UnsupportedValueType { 
                value_type: e 
            })?;
            
            Ok(crate::ast::FieldPattern::String {
                matcher: Arc::from(matcher),
                pattern_desc: Arc::from("null"),
            })
        }
        _ => Err(ParseError::UnsupportedValueType { 
            value_type: format!("{:?}", value) 
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[tokio::test]
    async fn test_build_tree_from_rule() -> Result<(), Box<dyn std::error::Error>> {
        let yaml = r#"
title: Test Rule
id: test-123
detection:
  selection:
    EventID: 1
  condition: selection
        "#;
        
        let rule = crate::rule::rule_from_yaml(yaml.as_bytes())?;
        let rule_handle = RuleHandle::new(rule, PathBuf::from("test.yml"));
        
        let _tree = build_tree(rule_handle).await?;
        
        // Tree builds successfully - comprehensive integration tests are in tests/
        Ok(())
    }
    
    #[tokio::test]
    async fn test_build_tree_with_complex_condition() -> Result<(), Box<dyn std::error::Error>> {
        let yaml = r#"
title: Complex Rule
id: test-456
detection:
  selection1:
    EventID: 1
  selection2:
    User: admin
  condition: selection1 and selection2
        "#;
        
        let rule = crate::rule::rule_from_yaml(yaml.as_bytes())?;
        let rule_handle = RuleHandle::new(rule, PathBuf::from("test.yml"));
        
        let tree = build_tree(rule_handle).await?;
        
        // Tree builds successfully - comprehensive integration tests are in tests/
        Ok(())
    }
    
    #[test]
    fn test_recursion_limit() {
        let detection = Detection::new();
        
        // Create deeply nested tokens that would exceed recursion limit
        let mut tokens = Vec::new();
        for _ in 0..60 {
            tokens.push(Item::new(Token::SepLpar, "(".to_string()));
        }
        tokens.push(Item::new(Token::Identifier, "test".to_string()));
        for _ in 0..60 {
            tokens.push(Item::new(Token::SepRpar, ")".to_string()));
        }
        
        let result = build_branch(&detection, tokens, 0, false);
        
        match result {
            Err(ParseError::RecursionLimitExceeded { current, limit }) => {
                assert!(current > limit);
                assert_eq!(limit, MAX_RECURSION_DEPTH);
            }
            _ => panic!("Expected RecursionLimitExceeded error"),
        }
    }
    
    #[test]
    fn test_glob_pattern_validation() {
        // Test pattern with too many wildcards
        let result = validate_glob_pattern("*?*?*?*?*?*?*?*?*?*?*");
        assert!(result.is_err());
        
        // Test pathological patterns
        let result = validate_glob_pattern("**test");
        assert!(result.is_err());
        
        let result = validate_glob_pattern("*?*test");
        assert!(result.is_err());
        
        // Test valid pattern
        let result = validate_glob_pattern("test*file?");
        assert!(result.is_ok());
    }
}
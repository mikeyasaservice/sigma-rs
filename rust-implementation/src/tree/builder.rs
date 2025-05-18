use std::sync::Arc;
use globset::GlobBuilder;

use crate::ast::{Branch, nodes::{NodeAnd, NodeOr, NodeNot, NodeSimpleAnd, NodeSimpleOr, Identifier}, FieldRule, FieldPattern};
use crate::pattern::IdentifierType;
use crate::lexer::{Token, Item};
use crate::parser::{Parser, ParseError};
use crate::rule::{Detection, RuleHandle};
use crate::tree::Tree;

/// Build a new Tree from a RuleHandle
pub async fn build_tree(rule: RuleHandle) -> Result<Tree, ParseError> {
    let condition = rule.rule.detection.condition()
        .ok_or_else(|| ParseError::MissingCondition)?;
    
    // Create parser with detection
    let mut parser = Parser::new(rule.rule.detection.clone(), rule.no_collapse_ws);
    
    // Run parser
    parser.run().await?;
    
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
    let mut iter = tokens.into_iter().peekable();
    
    let mut and_nodes: Vec<Arc<dyn Branch>> = Vec::new();
    let mut or_nodes: Vec<Arc<dyn Branch>> = Vec::new();
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
                let and_branch = reduce_branches(and_nodes, BranchType::And);
                or_nodes.push(and_branch);
                and_nodes = Vec::new();
            }
            
            Token::KeywordNot => {
                negated = true;
            }
            
            Token::SepLpar => {
                // Extract group and build recursively
                let group_tokens = extract_group(&mut iter)?;
                let branch = build_branch(detection, group_tokens, depth + 1, no_collapse_ws)?;
                
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
                    reduce_branches(rules, BranchType::And)
                } else {
                    reduce_branches(rules, BranchType::Or)
                };
                
                and_nodes.push(if negated {
                    Arc::new(NodeNot::new(branch))
                } else {
                    branch
                });
                negated = false;
            }
            
            Token::IdentifierWithWildcard => {
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
                    reduce_branches(rules, BranchType::And)
                } else {
                    reduce_branches(rules, BranchType::Or)
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
    
    // Final reduction
    let and_branch = reduce_branches(and_nodes, BranchType::And);
    or_nodes.push(and_branch);
    
    if negated {
        Ok(Arc::new(NodeNot::new(or_nodes.pop().unwrap())))
    } else {
        Ok(reduce_branches(or_nodes, BranchType::Or))
    }
}

enum BranchType {
    And,
    Or,
}

fn reduce_branches(mut branches: Vec<Arc<dyn Branch>>, branch_type: BranchType) -> Arc<dyn Branch> {
    match branches.len() {
        0 => panic!("Cannot reduce empty branch list"),
        1 => branches.pop().unwrap(),
        2 => {
            let right = branches.pop().unwrap();
            let left = branches.pop().unwrap();
            match branch_type {
                BranchType::And => Arc::new(NodeAnd::new(left, right)),
                BranchType::Or => Arc::new(NodeOr::new(left, right)),
            }
        }
        _ => {
            match branch_type {
                BranchType::And => Arc::new(NodeSimpleAnd::new(branches)),
                BranchType::Or => Arc::new(NodeSimpleOr::new(branches)),
            }
        }
    }
}

fn extract_group(iter: &mut std::iter::Peekable<std::vec::IntoIter<Item>>) -> Result<Vec<Item>, ParseError> {
    let mut group = Vec::new();
    let mut balance = 1;
    
    while let Some(item) = iter.next() {
        if balance > 0 {
            group.push(item.clone());
        }
        
        match item.token {
            Token::SepLpar => balance += 1,
            Token::SepRpar => {
                balance -= 1;
                if balance == 0 {
                    group.pop(); // Remove the closing paren
                    return Ok(group);
                }
            }
            _ => {}
        }
    }
    
    Err(ParseError::UnmatchedParenthesis)
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
    let mut rules = Vec::new();
    
    for (key, value) in detection.iter() {
        if glob.is_match(key) {
            let branch = build_rule_from_ident(value, IdentifierType::Selection, no_collapse_ws)?;
            rules.push(branch);
        }
    }
    
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
                        "keywords".to_string(),
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
                // For single field selections, create a simple field rule
                if obj.len() == 1 {
                    let (key, val) = obj.iter().next().unwrap();
                    let pattern = create_field_pattern(val)?;
                    let field_rule = crate::ast::FieldRule::new(key.clone(), pattern);
                    return Ok(Arc::new(Identifier::from_rule(field_rule)));
                }
                
                // For multiple fields, create an AND of field rules
                let branches: Vec<Arc<dyn Branch>> = obj
                    .iter()
                    .map(|(key, val)| {
                        let pattern = create_field_pattern(val)?;
                        let field_rule = crate::ast::FieldRule::new(key.clone(), pattern);
                        Ok(Arc::new(Identifier::from_rule(field_rule)) as Arc<dyn Branch>)
                    })
                    .collect::<Result<Vec<_>, ParseError>>()?;
                
                Ok(reduce_branches(branches, BranchType::And))
            } else {
                Err(ParseError::InvalidSelectionConstruct)
            }
        }
    }
}

fn create_field_pattern(value: &serde_json::Value) -> Result<crate::ast::FieldPattern, ParseError> {
    match value {
        serde_json::Value::String(s) => {
            // Check if it's a glob pattern
            if s.contains('*') || s.contains('?') {
                Ok(crate::ast::FieldPattern::Glob(s.clone()))
            } else {
                Ok(crate::ast::FieldPattern::Exact(s.clone()))
            }
        }
        serde_json::Value::Number(n) => {
            Ok(crate::ast::FieldPattern::Exact(n.to_string()))
        }
        serde_json::Value::Bool(b) => {
            Ok(crate::ast::FieldPattern::Exact(b.to_string()))
        }
        serde_json::Value::Null => {
            Ok(crate::ast::FieldPattern::Exact("null".to_string()))
        }
        _ => Err(ParseError::UnsupportedValueType { 
            value_type: format!("{:?}", value) 
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::Rule;
    use std::path::PathBuf;
    
    #[tokio::test]
    async fn test_build_tree_from_rule() {
        let yaml = r#"
title: Test Rule
id: test-123
detection:
  selection:
    EventID: 1
  condition: selection
        "#;
        
        let rule = crate::rule::rule_from_yaml(yaml.as_bytes()).unwrap();
        let rule_handle = RuleHandle::new(rule, PathBuf::from("test.yml"));
        
        let tree = build_tree(rule_handle).await.unwrap();
        
        // TODO: Add integration test with proper Event implementation
        // This test verifies that the tree builds successfully
    }
    
    #[tokio::test]
    async fn test_build_tree_with_complex_condition() {
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
        
        let rule = crate::rule::rule_from_yaml(yaml.as_bytes()).unwrap();
        let rule_handle = RuleHandle::new(rule, PathBuf::from("test.yml"));
        
        let tree = build_tree(rule_handle).await.unwrap();
        
        // TODO: Add integration test with proper Event implementation  
        // This test verifies that the tree builds successfully
    }
}
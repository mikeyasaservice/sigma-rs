use crate::ast::{Branch, FieldPattern, FieldRule, NodeSimpleAnd, NodeSimpleOr};
use crate::detection::Detection;
use crate::lexer::{Item, Lexer};
use crate::lexer::token::Token;
use crate::parser::validate::valid_token_sequence;
use std::sync::Arc;
use tokio::sync::mpsc;

mod error;
mod validate;

pub use error::ParseError;

/// Parser for Sigma rules that consumes tokens from lexer and builds AST
pub struct Parser {
    lexer: Option<Lexer>,
    tokens: Vec<Item>,
    previous: Option<Item>,
    sigma: Detection,
    condition: String,
    result: Option<Arc<dyn Branch>>,
    no_collapse_ws: bool,
}

impl Parser {
    /// Create a new parser with the given lexer and detection configuration
    pub fn new(lexer: Lexer, sigma: Detection, no_collapse_ws: bool) -> Self {
        let condition = sigma.condition().unwrap_or("").to_string();
        Self {
            lexer: Some(lexer),
            tokens: Vec::new(),
            previous: None,
            sigma,
            condition,
            result: None,
            no_collapse_ws,
        }
    }

    /// Run the parser, collecting tokens and building the AST
    pub async fn run(&mut self) -> Result<(), ParseError> {
        if self.lexer.is_none() {
            return Err(ParseError::LexerNotInitialized);
        }

        // Pass 1: collect tokens and validate sequences
        self.collect().await?;

        // Pass 2: build AST from tokens
        self.parse()?;

        Ok(())
    }

    /// Collect tokens from lexer and validate sequences
    async fn collect(&mut self) -> Result<(), ParseError> {
        let lexer = self.lexer.take().ok_or(ParseError::LexerNotInitialized)?;
        let mut receiver = lexer.scan().await;

        self.previous = Some(Item {
            token: Token::Begin,
            value: String::new(),
        });

        while let Some(item) = receiver.recv().await {
            // Check for unsupported tokens
            if item.token == Token::Unsupported {
                return Err(ParseError::unsupported_token(&item.value));
            }

            // Validate token sequence
            if let Some(prev) = &self.previous {
                if prev.token != Token::Begin
                    && !valid_token_sequence(prev.token, item.token)
                {
                    return Err(ParseError::invalid_sequence(
                        prev.clone(),
                        item.clone(),
                        self.tokens.clone(),
                    ));
                }
            }

            // Don't collect EOF token
            if item.token != Token::Eof {
                self.tokens.push(item.clone());
            }

            self.previous = Some(item);
        }

        // Validate final token
        if let Some(last) = &self.previous {
            if last.token != Token::Eof {
                return Err(ParseError::incomplete_sequence(
                    self.condition.clone(),
                    self.tokens.clone(),
                    last.clone(),
                ));
            }
        }

        Ok(())
    }

    /// Parse collected tokens into AST
    fn parse(&mut self) -> Result<(), ParseError> {
        let result = new_branch(
            &self.sigma,
            &self.tokens,
            0,
            self.no_collapse_ws,
        )?;
        self.result = Some(result);
        Ok(())
    }

    /// Get the resulting AST
    pub fn result(&self) -> Option<Arc<dyn Branch>> {
        self.result.clone()
    }

    /// Get the parsed tokens (for debugging)
    pub fn tokens(&self) -> &[Item] {
        &self.tokens
    }
}

/// Build a branch of the AST from tokens
fn new_branch(
    detection: &Detection,
    tokens: &[Item],
    depth: usize,
    no_collapse_ws: bool,
) -> Result<Arc<dyn Branch>, ParseError> {
    let mut token_iter = tokens.iter().peekable();
    let mut and_branches: Vec<Arc<dyn Branch>> = Vec::new();
    let mut or_branches: Vec<Arc<dyn Branch>> = Vec::new();
    let mut negated = false;
    let mut wildcard: Option<Token> = None;

    while let Some(item) = token_iter.next() {
        match item.token {
            Token::Identifier => {
                let value = detection.get(&item.value)
                    .ok_or_else(|| ParseError::missing_condition_item(&item.value))?;
                    
                // Create a field rule from the identifier and value
                let rule = create_rule_from_ident(&item.value, value, no_collapse_ws)?;
                let branch = if negated {
                    Arc::new(crate::ast::NodeNot::new(rule)) as Arc<dyn Branch>
                } else {
                    rule
                };
                and_branches.push(branch);
                negated = false;
            }
            
            Token::KeywordAnd => {
                // AND is implicit in collection
            }
            
            Token::KeywordOr => {
                // Finalize current AND group and start new one
                let and_node = NodeSimpleAnd::new(and_branches.clone()).reduce();
                or_branches.push(and_node);
                and_branches.clear();
            }
            
            Token::KeywordNot => {
                negated = true;
            }
            
            Token::SeparatorLeftParen => {
                // Extract grouped tokens and recursively parse
                let group_tokens = extract_group(&mut token_iter)?;
                let branch = new_branch(detection, &group_tokens, depth + 1, no_collapse_ws)?;
                let final_branch = if negated {
                    Arc::new(crate::ast::NodeNot::new(branch)) as Arc<dyn Branch>
                } else {
                    branch
                };
                and_branches.push(final_branch);
                negated = false;
            }
            
            Token::StmtAll => {
                wildcard = Some(Token::StmtAll);
            }
            
            Token::StmtOne => {
                wildcard = Some(Token::StmtOne);
            }
            
            Token::IdentifierAll => {
                // Handle "all of them" or "1 of them"
                let branches = extract_all_to_rules(detection, no_collapse_ws)?;
                let node: Arc<dyn Branch> = match wildcard {
                    Some(Token::StmtAll) => Arc::new(NodeSimpleAnd::new(branches)),
                    Some(Token::StmtOne) => Arc::new(NodeSimpleOr::new(branches)),
                    _ => return Err(ParseError::parser_error("Invalid wildcard context")),
                };
                let final_node = if negated {
                    Arc::new(crate::ast::NodeNot::new(node))
                } else {
                    node
                };
                and_branches.push(final_node);
                negated = false;
                wildcard = None;
            }
            
            Token::IdentifierWithWildcard => {
                // Handle wildcard patterns like "selection*"
                let pattern = glob::Pattern::new(&item.value)
                    .map_err(|e| ParseError::WildcardCompilationError(e.to_string()))?;
                    
                let matching_branches = extract_wildcard_idents(detection, &pattern, no_collapse_ws)?;
                let node: Arc<dyn Branch> = match wildcard {
                    Some(Token::StmtAll) => Arc::new(NodeSimpleAnd::new(matching_branches)),
                    Some(Token::StmtOne) => Arc::new(NodeSimpleOr::new(matching_branches)),
                    _ => return Err(ParseError::parser_error("Invalid wildcard context")),
                };
                let final_node = if negated {
                    Arc::new(crate::ast::NodeNot::new(node))
                } else {
                    node
                };
                and_branches.push(final_node);
                negated = false;
                wildcard = None;
            }
            
            _ => {
                return Err(ParseError::unsupported_token(&item.value));
            }
        }
    }

    // Finalize any remaining AND branches
    if !and_branches.is_empty() {
        let and_node = NodeSimpleAnd::new(and_branches).reduce();
        or_branches.push(and_node);
    }

    // Return final OR node
    Ok(NodeSimpleOr::new(or_branches).reduce())
}

/// Extract tokens within a group (parentheses)
fn extract_group<'a, I>(iter: &mut std::iter::Peekable<I>) -> Result<Vec<Item>, ParseError>
where
    I: Iterator<Item = &'a Item>,
{
    let mut group = Vec::new();
    let mut balance = 1;

    while let Some(item) = iter.next() {
        if balance > 0 {
            group.push(item.clone());
        }

        match item.token {
            Token::SeparatorLeftParen => balance += 1,
            Token::SeparatorRightParen => {
                balance -= 1;
                if balance == 0 {
                    // Remove the closing paren from the group
                    group.pop();
                    return Ok(group);
                }
            }
            _ => {}
        }
    }

    Err(ParseError::parser_error("Unbalanced parentheses"))
}

/// Create a field rule from an identifier and value
fn create_rule_from_ident(
    field: &str,
    value: &serde_json::Value,
    no_collapse_ws: bool,
) -> Result<Arc<dyn Branch>, ParseError> {
    // TODO: Implement proper rule creation from JSON value
    // For now, create a simple exact match rule
    let pattern = match value {
        serde_json::Value::String(s) => FieldPattern::Exact(s.clone()),
        serde_json::Value::Array(arr) => {
            // Handle array of values as OR
            let branches: Vec<Arc<dyn Branch>> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| {
                    Arc::new(FieldRule::new(
                        field.to_string(),
                        FieldPattern::Exact(s.to_string()),
                    )) as Arc<dyn Branch>
                })
                .collect();
            return Ok(Arc::new(NodeSimpleOr::new(branches)));
        }
        _ => return Err(ParseError::parser_error("Unsupported value type")),
    };

    Ok(Arc::new(FieldRule::new(field.to_string(), pattern)))
}

/// Extract all fields (except condition) and create rules
fn extract_all_to_rules(
    detection: &Detection,
    no_collapse_ws: bool,
) -> Result<Vec<Arc<dyn Branch>>, ParseError> {
    let mut rules = Vec::new();
    
    for (key, value) in detection.extract() {
        let rule = create_rule_from_ident(&key, &value, no_collapse_ws)?;
        rules.push(rule);
    }
    
    Ok(rules)
}

/// Extract identifiers matching a wildcard pattern
fn extract_wildcard_idents(
    detection: &Detection,
    pattern: &glob::Pattern,
    no_collapse_ws: bool,
) -> Result<Vec<Arc<dyn Branch>>, ParseError> {
    let mut rules = Vec::new();
    
    for (key, value) in detection.iter() {
        if key != "condition" && pattern.matches(key) {
            let rule = create_rule_from_ident(key, value, no_collapse_ws)?;
            rules.push(rule);
        }
    }
    
    if rules.is_empty() {
        return Err(ParseError::parser_error("No matching identifiers for wildcard pattern"));
    }
    
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_basic_parser() {
        let condition = "selection1 and selection2";
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection1".to_string(), serde_json::json!({"field1": "value1"}));
        detection.insert("selection2".to_string(), serde_json::json!({"field2": "value2"}));

        let lexer = Lexer::new(condition.to_string());
        let mut parser = Parser::new(lexer, detection, false);
        
        let result = parser.run().await;
        assert!(result.is_ok());
        assert!(parser.result().is_some());
    }
}

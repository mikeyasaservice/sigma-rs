use crate::ast::{Branch, FieldPattern, FieldRule, NodeSimpleAnd, NodeSimpleOr};
use crate::detection::Detection;
use crate::lexer::Lexer;
use crate::lexer::token::{Token, Item};
use crate::parser::validate::valid_token_sequence;
use std::sync::Arc;
use tokio::task;

pub mod error;
pub mod validate;

pub use error::ParseError;

/// Parser for Sigma rules that consumes tokens from lexer and builds AST
#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Item>,
    previous: Option<Item>,
    sigma: Detection,
    condition: String,
    result: Option<Arc<dyn Branch>>,
    no_collapse_ws: bool,
}

impl Parser {
    /// Create a new parser with detection configuration
    pub fn new(sigma: Detection, no_collapse_ws: bool) -> Self {
        let condition = sigma.condition().unwrap_or("").to_string();
        Self {
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
        // Pass 1: collect tokens and validate sequences
        self.collect().await?;

        // Pass 2: build AST from tokens
        self.parse()?;

        Ok(())
    }

    /// Collect tokens from lexer and validate sequences
    async fn collect(&mut self) -> Result<(), ParseError> {
        let (lexer, mut rx) = Lexer::new(self.condition.clone());
        
        // Start lexer in background
        let lexer_handle = task::spawn(async move {
            lexer.scan().await
        });

        self.previous = Some(Item::new(
            Token::Identifier, // Placeholder, will be ignored
            String::from("<begin>"),
        ));

        while let Some(item) = rx.recv().await {
            // Check for unsupported tokens
            if item.token == Token::Unsupported {
                return Err(ParseError::unsupported_token(&item.value));
            }

            // Validate token sequence
            if let Some(prev) = &self.previous {
                // Special handling for begin state
                let is_begin = prev.value == "<begin>";
                
                if !is_begin && !valid_token_sequence(prev.token, item.token) {
                    return Err(ParseError::invalid_sequence(
                        prev.clone(),
                        item.clone(),
                        self.tokens.clone(),
                    ));
                }
            }

            // Don't collect EOF token
            if item.token != Token::LitEof {
                self.tokens.push(item.clone());
            }

            self.previous = Some(item);
        }

        // Wait for lexer to complete
        lexer_handle.await.map_err(|e| ParseError::parser_error(e.to_string()))?
            .map_err(|e| ParseError::parser_error(e.to_string()))?;

        // Validate final token
        if let Some(last) = &self.previous {
            if last.token != Token::LitEof {
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
                    
                // Debug logging for complex object processing
                if value.is_object() {
                    eprintln!("Debug: Processing object detection rule '{}': {:?}", &item.value, value);
                }
                
                // Create a field rule from the identifier and value
                let rule = create_rule_from_ident(&item.value, value, no_collapse_ws)?;
                let branch = if negated {
                    eprintln!("Debug: Applying NOT to rule '{}'", &item.value);
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
            
            Token::SepLpar => {
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
            
            Token::StmtAllOf => {
                wildcard = Some(Token::StmtAllOf);
            }
            
            Token::StmtOneOf => {
                wildcard = Some(Token::StmtOneOf);
            }
            
            Token::IdentifierAll => {
                // Handle "all of them" or "1 of them"
                let branches = extract_all_to_rules(detection, no_collapse_ws)?;
                let node: Arc<dyn Branch> = match wildcard {
                    Some(Token::StmtAllOf) => Arc::new(NodeSimpleAnd::new(branches)),
                    Some(Token::StmtOneOf) => Arc::new(NodeSimpleOr::new(branches)),
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
                    Some(Token::StmtAllOf) => Arc::new(NodeSimpleAnd::new(matching_branches)),
                    Some(Token::StmtOneOf) => Arc::new(NodeSimpleOr::new(matching_branches)),
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
    if or_branches.is_empty() {
        return Err(ParseError::parser_error("No valid branches found"));
    }
    
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
            Token::SepLpar => balance += 1,
            Token::SepRpar => {
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
    // Handle different value types
    match value {
        serde_json::Value::String(s) => {
            Ok(Arc::new(FieldRule::new(
                field.to_string(),
                FieldPattern::Exact(process_string_value(s, no_collapse_ws)),
            )))
        }
        serde_json::Value::Number(n) => {
            // Convert number to string for field matching
            Ok(Arc::new(FieldRule::new(
                field.to_string(),
                FieldPattern::Exact(n.to_string()),
            )))
        }
        serde_json::Value::Bool(b) => {
            // Convert boolean to string for field matching
            Ok(Arc::new(FieldRule::new(
                field.to_string(),
                FieldPattern::Exact(b.to_string()),
            )))
        }
        serde_json::Value::Array(arr) => {
            // Handle array of values as OR
            let branches: Vec<Arc<dyn Branch>> = arr
                .iter()
                .filter_map(|v| match v {
                    serde_json::Value::String(s) => Some(Arc::new(FieldRule::new(
                        field.to_string(),
                        FieldPattern::Exact(process_string_value(s, no_collapse_ws)),
                    )) as Arc<dyn Branch>),
                    serde_json::Value::Number(n) => Some(Arc::new(FieldRule::new(
                        field.to_string(),
                        FieldPattern::Exact(n.to_string()),
                    )) as Arc<dyn Branch>),
                    serde_json::Value::Bool(b) => Some(Arc::new(FieldRule::new(
                        field.to_string(),
                        FieldPattern::Exact(b.to_string()),
                    )) as Arc<dyn Branch>),
                    _ => None,
                })
                .collect();
            
            if branches.is_empty() {
                return Err(ParseError::parser_error("Empty array in field rule"));
            }
            
            Ok(NodeSimpleOr::new(branches).reduce())
        }
        serde_json::Value::Object(obj) => {
            // Handle complex field definitions
            create_complex_field_rule(field, obj, no_collapse_ws)
        }
        _ => Err(ParseError::parser_error(&format!(
            "Unsupported value type for field rule: {} -> {:?}", 
            field, value
        ))),
    }
}

/// Process string value based on whitespace collapse settings
fn process_string_value(value: &str, no_collapse_ws: bool) -> String {
    if no_collapse_ws {
        value.to_string()
    } else {
        // Collapse multiple whitespaces to single space
        value.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// Create a complex field rule from an object definition  
fn create_complex_field_rule(
    field: &str,
    obj: &serde_json::Map<String, serde_json::Value>,
    no_collapse_ws: bool,
) -> Result<Arc<dyn Branch>, ParseError> {
    // For object definitions, we create field rules directly from the object's properties
    // This handles cases like: "selection": { "EventID": 4688, "Process": "cmd.exe" }
    let mut branches: Vec<Arc<dyn Branch>> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    
    for (key, value) in obj.iter() {
        // Use the key directly as the field name, not prepended with parent field
        match create_rule_from_ident(key, value, no_collapse_ws) {
            Ok(branch) => branches.push(branch),
            Err(e) => errors.push(format!("Error creating rule for '{}': {}", key, e)),
        }
    }
    
    if branches.is_empty() {
        if !errors.is_empty() {
            return Err(ParseError::parser_error(&format!(
                "Failed to create complex field rule for '{}'. Errors: {}",
                field,
                errors.join("; ")
            )));
        }
        return Err(ParseError::parser_error(&format!(
            "Invalid complex field rule for '{}': no valid branches created",
            field
        )));
    }
    
    // Log warnings if some fields failed but we have at least one valid branch
    if !errors.is_empty() {
        eprintln!("Warning: Some fields in '{}' failed to parse: {}", field, errors.join("; "));
    }
    
    Ok(NodeSimpleAnd::new(branches).reduce())
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
    
    if rules.is_empty() {
        return Err(ParseError::parser_error("No detection fields found"));
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
    use crate::detection::Detection;

    #[tokio::test]
    async fn test_basic_parser() {
        let condition = "selection1 and selection2";
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection1".to_string(), serde_json::json!("value1"));
        detection.insert("selection2".to_string(), serde_json::json!("value2"));

        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        assert!(result.is_ok());
        assert!(parser.result().is_some());
    }

    #[tokio::test]
    async fn test_parser_with_or() {
        let condition = "selection1 or selection2";
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection1".to_string(), serde_json::json!("value1"));
        detection.insert("selection2".to_string(), serde_json::json!("value2"));

        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        assert!(result.is_ok());
        assert!(parser.result().is_some());
    }

    #[tokio::test]
    async fn test_parser_with_not() {
        let condition = "selection1 and not selection2";
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection1".to_string(), serde_json::json!("value1"));
        detection.insert("selection2".to_string(), serde_json::json!("value2"));

        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        assert!(result.is_ok());
        assert!(parser.result().is_some());
    }

    #[tokio::test]
    async fn test_parser_with_parentheses() {
        let condition = "(selection1 or selection2) and selection3";
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection1".to_string(), serde_json::json!("value1"));
        detection.insert("selection2".to_string(), serde_json::json!("value2"));
        detection.insert("selection3".to_string(), serde_json::json!("value3"));

        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        assert!(result.is_ok());
        assert!(parser.result().is_some());
    }

    #[tokio::test]
    async fn test_parser_with_object_rules() {
        let condition = "selection and not exclusion";
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection".to_string(), serde_json::json!({
            "EventID": 4688,
            "Process": "cmd.exe"
        }));
        detection.insert("exclusion".to_string(), serde_json::json!({
            "User": "SYSTEM"
        }));

        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        assert!(result.is_ok(), "Parser should handle object rules correctly");
        assert!(parser.result().is_some());
    }

    #[tokio::test]
    async fn test_parser_with_numeric_values() {
        let condition = "selection";
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection".to_string(), serde_json::json!({
            "EventID": 4688,
            "ProcessId": 1234,
            "Enabled": true
        }));

        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        assert!(result.is_ok(), "Parser should handle numeric and boolean values");
        assert!(parser.result().is_some());
    }
}
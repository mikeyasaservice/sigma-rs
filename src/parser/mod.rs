use crate::ast::{Branch, FieldPattern, FieldRule, NodeSimpleAnd, NodeSimpleOr};
use crate::rule::Detection;
use crate::lexer::Lexer;
use crate::lexer::token::{Token, Item};
use crate::parser::validate::valid_token_sequence;
use std::sync::Arc;
use tokio::task;
use tracing;

/// Parser error types
pub mod error;
/// Validation utilities for parsed rules
pub mod validate;

pub use error::ParseError;

/// Maximum number of tokens allowed in a single rule condition
const MAX_TOKENS: usize = 10_000;

/// Maximum recursion depth for nested expressions (reduced for safety)
const MAX_RECURSION_DEPTH: usize = 50;

/// Maximum memory allowed for token collection (10MB)
const MAX_MEMORY_BYTES: usize = 10 * 1024 * 1024;

/// Parser for Sigma rules that consumes tokens from lexer and builds AST
#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Item>,
    previous: Option<Item>,
    sigma: Detection,
    condition: Arc<str>,
    result: Option<Arc<dyn Branch>>,
    no_collapse_ws: bool,
    max_tokens: usize,
    memory_used: usize,
    max_memory: usize,
}

impl Parser {
    /// Create a new parser with detection configuration
    pub fn new(sigma: Detection, no_collapse_ws: bool) -> Self {
        let condition = Arc::from(sigma.condition().unwrap_or(""));
        Self {
            tokens: Vec::new(),
            previous: None,
            sigma,
            condition,
            result: None,
            no_collapse_ws,
            max_tokens: MAX_TOKENS,
            memory_used: 0,
            max_memory: MAX_MEMORY_BYTES,
        }
    }
    
    /// Create a new parser with custom token limit
    pub fn new_with_limits(sigma: Detection, no_collapse_ws: bool, max_tokens: usize) -> Self {
        let condition = Arc::from(sigma.condition().unwrap_or(""));
        Self {
            tokens: Vec::new(),
            previous: None,
            sigma,
            condition,
            result: None,
            no_collapse_ws,
            max_tokens,
            memory_used: 0,
            max_memory: MAX_MEMORY_BYTES,
        }
    }
    
    /// Estimate memory usage for an Item
    fn estimate_item_size(item: &Item) -> usize {
        // Base struct size + string value size + overhead
        std::mem::size_of::<Item>() + item.value.len() + 32
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
        let (lexer, mut rx) = Lexer::new(&self.condition);
        
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
                        &self.tokens,
                    ));
                }
            }

            // Don't collect EOF token
            if item.token != Token::LitEof {
                // Check token limit before adding
                if self.tokens.len() >= self.max_tokens {
                    return Err(ParseError::TokenLimitExceeded {
                        current: self.tokens.len(),
                        limit: self.max_tokens,
                    });
                }
                
                // Check memory limit before adding
                let item_size = Self::estimate_item_size(&item);
                if self.memory_used + item_size > self.max_memory {
                    return Err(ParseError::MemoryLimitExceeded {
                        current_bytes: self.memory_used + item_size,
                        limit_bytes: self.max_memory,
                    });
                }
                
                self.tokens.push(item.clone());
                self.memory_used += item_size;
            }

            self.previous = Some(item);
        }

        // Wait for lexer to complete
        lexer_handle.await
            .map_err(|e| ParseError::TaskJoinError(e.to_string()))?
            .map_err(|e| ParseError::LexerError(Arc::new(e)))?;

        // Validate final token
        if let Some(last) = &self.previous {
            if last.token != Token::LitEof {
                // Create a limited view of tokens for error context
                let token_count = self.tokens.len();
                let context_start = token_count.saturating_sub(10);
                let context_tokens = self.tokens[context_start..].to_vec();
                
                return Err(ParseError::incomplete_sequence(
                    self.condition.to_string(),
                    context_tokens,
                    last.clone(),
                ));
            }
        }

        Ok(())
    }

    /// Parse collected tokens into AST
    fn parse(&mut self) -> Result<(), ParseError> {
        // Pre-validate parentheses balance and depth
        self.validate_parentheses()?;
        
        let result = new_branch(
            &self.sigma,
            &self.tokens,
            0,
            self.no_collapse_ws,
        )?;
        self.result = Some(result);
        Ok(())
    }
    
    /// Validate parentheses are balanced and not too deeply nested
    fn validate_parentheses(&self) -> Result<(), ParseError> {
        let mut depth = 0;
        let mut max_depth = 0;
        
        for token in &self.tokens {
            match token.token {
                Token::SepLpar => {
                    depth += 1;
                    max_depth = max_depth.max(depth);
                    if max_depth > MAX_RECURSION_DEPTH {
                        return Err(ParseError::RecursionLimitExceeded {
                            current: max_depth,
                            limit: MAX_RECURSION_DEPTH,
                        });
                    }
                }
                Token::SepRpar => {
                    if depth == 0 {
                        return Err(ParseError::parser_error("Unmatched closing parenthesis"));
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        
        if depth != 0 {
            return Err(ParseError::parser_error("Unbalanced parentheses"));
        }
        
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
    // Check recursion depth limit
    if depth > MAX_RECURSION_DEPTH {
        return Err(ParseError::RecursionLimitExceeded {
            current: depth,
            limit: MAX_RECURSION_DEPTH,
        });
    }

    let mut token_iter = tokens.iter().peekable();
    
    // Estimate capacity based on token analysis for better performance
    let identifier_count = tokens.iter().filter(|t| t.token == Token::Identifier).count();
    let and_count = tokens.iter().filter(|t| matches!(t.token, Token::KeywordAnd)).count();
    let or_count = tokens.iter().filter(|t| matches!(t.token, Token::KeywordOr)).count();
    
    let estimated_and_branches = (and_count + identifier_count / 2).max(1);
    let estimated_or_branches = (or_count + identifier_count / 3).max(1);
    
    let mut and_branches: Vec<Arc<dyn Branch>> = Vec::with_capacity(estimated_and_branches);
    let mut or_branches: Vec<Arc<dyn Branch>> = Vec::with_capacity(estimated_or_branches);
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
                let and_node = NodeSimpleAnd::new(std::mem::take(&mut and_branches)).reduce()
                    .map_err(|e| ParseError::parser_error(e.to_string()))?;
                or_branches.push(and_node);
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
        let and_node = NodeSimpleAnd::new(and_branches).reduce()
            .map_err(|e| ParseError::parser_error(e.to_string()))?;
        or_branches.push(and_node);
    }

    // Return final OR node
    if or_branches.is_empty() {
        return Err(ParseError::parser_error("No valid branches found"));
    }
    
    NodeSimpleOr::new(or_branches).reduce()
        .map_err(|e| ParseError::parser_error(e.to_string()))
}

/// Extract tokens within a group (parentheses)
fn extract_group<'a, I>(iter: &mut std::iter::Peekable<I>) -> Result<Vec<Item>, ParseError>
where
    I: Iterator<Item = &'a Item>,
{
    let mut group = Vec::new();
    let mut balance = 1;

    for item in iter {
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

/// Parse field modifier from field string (e.g., "CommandLine|contains" -> ("CommandLine", Some(TextPatternModifier::Contains)))
fn parse_field_modifier(field: &str) -> (&str, Option<crate::pattern::TextPatternModifier>) {
    use crate::pattern::TextPatternModifier;
    
    if let Some(delimiter_pos) = field.find('|') {
        let field_name = &field[..delimiter_pos];
        let modifier_str = &field[delimiter_pos + 1..];
        
        let modifier = match modifier_str.to_lowercase().as_str() {
            "contains" => Some(TextPatternModifier::Contains),
            "prefix" | "startswith" => Some(TextPatternModifier::Prefix),
            "suffix" | "endswith" => Some(TextPatternModifier::Suffix),
            "all" => Some(TextPatternModifier::All),
            "re" | "regex" => Some(TextPatternModifier::Regex),
            "keyword" => Some(TextPatternModifier::Keyword),
            _ => None, // Unknown modifier, treat as none
        };
        
        (field_name, modifier)
    } else {
        (field, None)
    }
}

/// Create a field rule from an identifier and value
fn create_rule_from_ident(
    field: &str,
    value: &serde_json::Value,
    no_collapse_ws: bool,
) -> Result<Arc<dyn Branch>, ParseError> {
    use crate::pattern::{new_string_matcher, new_num_matcher, TextPatternModifier};
    
    // Parse field and modifier
    let (field_name, modifier) = parse_field_modifier(field);
    
    // Handle different value types
    match value {
        serde_json::Value::String(s) => {
            let processed = process_string_value(s, no_collapse_ws);
            let final_modifier = modifier.unwrap_or_else(|| {
                if processed.contains('*') || processed.contains('?') {
                    TextPatternModifier::None  // Will be handled as glob
                } else {
                    TextPatternModifier::None  // Exact match
                }
            });
            
            let matcher = new_string_matcher(
                final_modifier,
                false,  // lowercase
                false,  // all
                no_collapse_ws,
                vec![processed.clone()],
            ).map_err(|e| ParseError::string_pattern_creation_failed(
                field_name, 
                &processed, 
                e.to_string()
            ))?;
            
            Ok(Arc::new(FieldRule::new(
                Arc::from(field_name),
                FieldPattern::String {
                    matcher: Arc::from(matcher),
                    pattern_desc: Arc::from(processed),
                },
            )))
        }
        serde_json::Value::Number(n) => {
            if let Some(num) = n.as_i64() {
                let matcher = new_num_matcher(vec![num])
                    .map_err(|e| ParseError::numeric_pattern_creation_failed(
                        field_name, 
                        &n.to_string(), 
                        e.to_string()
                    ))?;
                
                Ok(Arc::new(FieldRule::new(
                    Arc::from(field_name),
                    FieldPattern::Numeric {
                        matcher: Arc::from(matcher),
                        pattern_desc: Arc::from(n.to_string()),
                    },
                )))
            } else {
                // Fall back to string matching for floats
                let matcher = new_string_matcher(
                    TextPatternModifier::None,
                    false,  // lowercase
                    false,  // all
                    no_collapse_ws,
                    vec![n.to_string()],
                ).map_err(|e| ParseError::string_pattern_creation_failed(
                    field_name, 
                    &n.to_string(), 
                    e.to_string()
                ))?;
                
                Ok(Arc::new(FieldRule::new(
                    Arc::from(field_name),
                    FieldPattern::String {
                        matcher: Arc::from(matcher),
                        pattern_desc: Arc::from(n.to_string()),
                    },
                )))
            }
        }
        serde_json::Value::Bool(b) => {
            let str_val = b.to_string();
            let matcher = new_string_matcher(
                TextPatternModifier::None,
                false,  // lowercase
                false,  // all
                no_collapse_ws,
                vec![str_val.clone()],
            ).map_err(|e| ParseError::string_pattern_creation_failed(
                field_name, 
                &str_val, 
                e.to_string()
            ))?;
            
            Ok(Arc::new(FieldRule::new(
                Arc::from(field_name),
                FieldPattern::String {
                    matcher: Arc::from(matcher),
                    pattern_desc: Arc::from(str_val),
                },
            )))
        }
        serde_json::Value::Array(arr) => {
            // Handle array of values as OR
            let mut branches: Vec<Arc<dyn Branch>> = Vec::new();
            let mut errors: Vec<String> = Vec::new();
            
            for v in arr.iter() {
                match v {
                    serde_json::Value::String(s) => {
                        let processed = process_string_value(s, no_collapse_ws);
                        let final_modifier = modifier.unwrap_or_else(|| {
                            if processed.contains('*') || processed.contains('?') {
                                TextPatternModifier::None  // Will be handled as glob
                            } else {
                                TextPatternModifier::None  // Exact match
                            }
                        });
                        
                        match new_string_matcher(
                            final_modifier,
                            false,  // lowercase
                            false,  // all
                            no_collapse_ws,
                            vec![processed.clone()],
                        ) {
                            Ok(matcher) => {
                                branches.push(Arc::new(FieldRule::new(
                                    Arc::from(field_name),
                                    FieldPattern::String {
                                        matcher: Arc::from(matcher),
                                        pattern_desc: Arc::from(processed),
                                    },
                                )) as Arc<dyn Branch>);
                            }
                            Err(e) => {
                                errors.push(format!("Failed to create string matcher for '{}': {}", processed, e));
                            }
                        }
                    }
                    serde_json::Value::Number(n) => {
                        if let Some(num) = n.as_i64() {
                            match new_num_matcher(vec![num]) {
                                Ok(matcher) => {
                                    branches.push(Arc::new(FieldRule::new(
                                        Arc::from(field_name),
                                        FieldPattern::Numeric {
                                            matcher: Arc::from(matcher),
                                            pattern_desc: Arc::from(n.to_string()),
                                        },
                                    )) as Arc<dyn Branch>);
                                }
                                Err(e) => {
                                    errors.push(format!("Failed to create numeric matcher for '{}': {}", n, e));
                                }
                            }
                        } else {
                            match new_string_matcher(
                                TextPatternModifier::None,
                                false,  // lowercase
                                false,  // all
                                no_collapse_ws,
                                vec![n.to_string()],
                            ) {
                                Ok(matcher) => {
                                    branches.push(Arc::new(FieldRule::new(
                                        Arc::from(field_name),
                                        FieldPattern::String {
                                            matcher: Arc::from(matcher),
                                            pattern_desc: Arc::from(n.to_string()),
                                        },
                                    )) as Arc<dyn Branch>);
                                }
                                Err(e) => {
                                    errors.push(format!("Failed to create string matcher for float '{}': {}", n, e));
                                }
                            }
                        }
                    }
                    serde_json::Value::Bool(b) => {
                        let str_val = b.to_string();
                        match new_string_matcher(
                            TextPatternModifier::None,
                            false,  // lowercase
                            false,  // all
                            no_collapse_ws,
                            vec![str_val.clone()],
                        ) {
                            Ok(matcher) => {
                                branches.push(Arc::new(FieldRule::new(
                                    Arc::from(field_name),
                                    FieldPattern::String {
                                        matcher: Arc::from(matcher),
                                        pattern_desc: Arc::from(str_val),
                                    },
                                )) as Arc<dyn Branch>);
                            }
                            Err(e) => {
                                errors.push(format!("Failed to create string matcher for boolean '{}': {}", str_val, e));
                            }
                        }
                    }
                    _ => {
                        errors.push(format!("Unsupported value type in array: {:?}", v));
                    }
                }
            }
            
            // Log errors but continue processing if we have at least some valid branches
            if !errors.is_empty() {
                tracing::warn!(
                    "Field '{}' had {} errors while processing array values: {}",
                    field_name,
                    errors.len(),
                    errors.join("; ")
                );
            }
            
            if branches.is_empty() {
                return Err(ParseError::no_valid_field_patterns(
                    "unknown", // Rule ID not available at this level
                    field_name,
                    errors,
                ));
            }
            
            NodeSimpleOr::new(branches).reduce()
                .map_err(|e| ParseError::parser_error(e.to_string()))
        }
        serde_json::Value::Object(obj) => {
            // Handle complex field definitions
            create_complex_field_rule(field, obj, no_collapse_ws)
        }
        _ => Err(ParseError::field_pattern_creation_failed(
            field, 
            &format!("{:?}", value), 
            "Unsupported value type for field rule"
        )),
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
            return Err(ParseError::no_valid_field_patterns(
                "unknown", // Rule ID not available at this level
                field,
                errors,
            ));
        }
        return Err(ParseError::field_pattern_creation_failed(
            field,
            "complex object",
            "no valid branches created from object properties"
        ));
    }
    
    // Log warnings if some fields failed but we have at least one valid branch
    
    NodeSimpleAnd::new(branches).reduce()
        .map_err(|e| ParseError::parser_error(e.to_string()))
}

/// Extract all fields (except condition) and create rules
fn extract_all_to_rules(
    detection: &Detection,
    no_collapse_ws: bool,
) -> Result<Vec<Arc<dyn Branch>>, ParseError> {
    let mut rules = Vec::new();
    let extracted = detection.extract();
    
    for (key, value) in extracted.iter() {
        let rule = create_rule_from_ident(key, value, no_collapse_ws)?;
        rules.push(rule);
    }
    
    if rules.is_empty() {
        return Err(ParseError::detection_parsing_failed(
            "unknown", // Rule ID not available at this level
            "No detection fields found in rule"
        ));
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
    use crate::rule::Detection;

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

    #[tokio::test]
    async fn test_parser_with_field_modifiers() {
        let condition = "selection";
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection".to_string(), serde_json::json!({
            "CommandLine|contains": "powershell",
            "ProcessName|prefix": "cmd",
            "ServiceName|suffix": ".exe"
        }));

        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        assert!(result.is_ok(), "Parser should handle field modifiers");
        assert!(parser.result().is_some());
    }

    #[tokio::test]
    async fn test_memory_limit() {
        // Create a large condition that would exceed memory limits
        let large_value = "x".repeat(1_000_000); // 1MB string
        let condition = format!("selection1 or selection2 or selection3 or selection4 or selection5");
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition));
        
        // Add selections with large values
        for i in 1..=5 {
            detection.insert(format!("selection{}", i), serde_json::json!(large_value.clone()));
        }
        
        let mut parser = Parser::new_with_limits(detection, false, 100);
        parser.max_memory = 1024 * 1024; // Set 1MB limit
        
        let result = parser.run().await;
        match result {
            Err(ParseError::MemoryLimitExceeded { .. }) => {
                // Expected error
            }
            _ => panic!("Expected MemoryLimitExceeded error"),
        }
    }

    #[tokio::test]
    async fn test_deep_recursion_protection() {
        // Create deeply nested parentheses
        let mut condition = String::new();
        for _ in 0..60 {
            condition.push('(');
        }
        condition.push_str("selection");
        for _ in 0..60 {
            condition.push(')');
        }
        
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition));
        detection.insert("selection".to_string(), serde_json::json!("value"));
        
        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        match result {
            Err(ParseError::RecursionLimitExceeded { .. }) => {
                // Expected error
            }
            _ => panic!("Expected RecursionLimitExceeded error"),
        }
    }

    #[tokio::test]
    async fn test_error_context_preservation() {
        // Create condition with invalid sequence
        let condition = "selection1 selection2"; // Missing operator
        let mut detection = Detection::new();
        detection.insert("condition".to_string(), serde_json::Value::String(condition.to_string()));
        detection.insert("selection1".to_string(), serde_json::json!("value1"));
        detection.insert("selection2".to_string(), serde_json::json!("value2"));
        
        let mut parser = Parser::new(detection, false);
        
        let result = parser.run().await;
        match result {
            Err(ParseError::InvalidTokenSequence { token_count, context_tokens, .. }) => {
                // Verify we have context without the full token list
                assert!(token_count > 0);
                assert!(context_tokens.len() <= 5); // Should have limited context
            }
            _ => panic!("Expected InvalidTokenSequence error"),
        }
    }
}
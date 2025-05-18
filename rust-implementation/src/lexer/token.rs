use std::borrow::Cow;
use glob::{Pattern, PatternError};

/// Token types in Sigma expressions
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    // Helpers for internal stuff
    Error = 0,
    Unsupported = 1,
    Nil = 2,

    // User-defined word
    Identifier = 3,
    IdentifierWithWildcard = 4,
    IdentifierAll = 5,

    // Literals
    LitEof = 6,

    // Separators
    SepLpar = 7,
    SepRpar = 8,
    SepPipe = 9,

    // Operators
    OpEq = 10,
    OpGt = 11,
    OpGte = 12,
    OpLt = 13,
    OpLte = 14,

    // Keywords
    KeywordAnd = 15,
    KeywordOr = 16,
    KeywordNot = 17,
    KeywordAgg = 18,

    // Statements
    StmtOneOf = 19,
    StmtAllOf = 20,
}

impl Token {
    /// Get the literal representation of the token
    pub fn literal(&self) -> &'static str {
        match self {
            Token::Identifier | Token::IdentifierWithWildcard => "keywords",
            Token::IdentifierAll => "them",
            Token::SepLpar => "(",
            Token::SepRpar => ")",
            Token::SepPipe => "|",
            Token::OpEq => "=",
            Token::OpGt => ">",
            Token::OpGte => ">=",
            Token::OpLt => "<",
            Token::OpLte => "<=",
            Token::KeywordAnd => "and",
            Token::KeywordOr => "or",
            Token::KeywordNot => "not",
            Token::StmtAllOf => "all of",
            Token::StmtOneOf => "1 of",
            Token::LitEof | Token::Nil => "",
            _ => "Err",
        }
    }

    /// Get the rune representation of separator tokens
    pub fn rune(&self) -> Option<char> {
        match self {
            Token::SepLpar => Some('('),
            Token::SepRpar => Some(')'),
            Token::SepPipe => Some('|'),
            _ => None,
        }
    }

    /// Check if this is a valid keyword token
    pub fn from_keyword(s: &str) -> Option<Token> {
        match s.to_lowercase().as_str() {
            "and" => Some(Token::KeywordAnd),
            "or" => Some(Token::KeywordOr),
            "not" => Some(Token::KeywordNot),
            "sum" | "min" | "max" | "count" | "avg" => Some(Token::KeywordAgg),
            "them" => Some(Token::IdentifierAll),
            "1 of" => Some(Token::StmtOneOf),
            _ => None,
        }
    }
}

/// Lexical token with its value
#[derive(Debug, Clone)]
pub struct Item {
    pub token: Token,
    pub value: String,
    glob_val: Option<Pattern>,
    glob_compile_failed: bool,
}

impl Item {
    /// Create a new item
    pub fn new(token: Token, value: String) -> Self {
        Self {
            token,
            value,
            glob_val: None,
            glob_compile_failed: false,
        }
    }

    /// Get the compiled glob pattern for this item
    pub fn glob(&mut self) -> Option<&Pattern> {
        if self.glob_val.is_none() && !self.glob_compile_failed {
            let pattern_str = escape_sigma_for_glob(&self.value);
            match Pattern::new(&pattern_str) {
                Ok(glob) => self.glob_val = Some(glob),
                Err(_) => {
                    self.glob_compile_failed = true;
                    return None;
                }
            }
        }
        self.glob_val.as_ref()
    }
}

/// Escape Sigma wildcards for glob patterns
fn escape_sigma_for_glob(s: &str) -> String {
    // This is a placeholder - implement the actual escaping logic
    // based on the Go version's escapeSigmaForGlob function
    s.to_string()
}

/// Check if a keyword is valid based on the given string
pub fn check_keyword(input: &str) -> Token {
    if input.is_empty() {
        return Token::Nil;
    }

    let lower = input.to_lowercase();
    match lower.as_str() {
        "and" => Token::KeywordAnd,
        "or" => Token::KeywordOr,
        "not" => Token::KeywordNot,
        "sum" | "min" | "max" | "count" | "avg" => Token::KeywordAgg,
        "them" => Token::IdentifierAll,
        "1 of" => Token::StmtOneOf,
        _ => {
            if input.contains('*') {
                Token::IdentifierWithWildcard
            } else {
                Token::Identifier
            }
        }
    }
}

/// Validate a token sequence
pub fn valid_token_sequence(t1: Token, t2: Token) -> bool {
    use Token::*;
    
    match t2 {
        StmtAllOf | StmtOneOf => matches!(
            t1,
            Token::Nil | SepLpar | KeywordAnd | KeywordOr | KeywordNot
        ),
        IdentifierAll => matches!(t1, StmtAllOf | StmtOneOf),
        Identifier | IdentifierWithWildcard => matches!(
            t1,
            SepLpar | Token::Nil | KeywordAnd | KeywordOr | KeywordNot | StmtOneOf | StmtAllOf
        ),
        KeywordAnd | KeywordOr => matches!(
            t1,
            Identifier | IdentifierAll | IdentifierWithWildcard | SepRpar
        ),
        KeywordNot => matches!(t1, KeywordAnd | KeywordOr | SepLpar | Token::Nil),
        SepLpar => matches!(t1, KeywordAnd | KeywordOr | KeywordNot | Token::Nil | SepLpar),
        SepRpar => matches!(
            t1,
            Identifier | IdentifierAll | IdentifierWithWildcard | SepLpar | SepRpar
        ),
        LitEof => matches!(
            t1,
            Identifier | IdentifierAll | IdentifierWithWildcard | SepRpar
        ),
        SepPipe => matches!(
            t1,
            Identifier | IdentifierAll | IdentifierWithWildcard | SepRpar
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_literal() {
        assert_eq!(Token::KeywordAnd.literal(), "and");
        assert_eq!(Token::SepLpar.literal(), "(");
        assert_eq!(Token::StmtOneOf.literal(), "1 of");
    }

    #[test]
    fn test_token_from_keyword() {
        assert_eq!(Token::from_keyword("and"), Some(Token::KeywordAnd));
        assert_eq!(Token::from_keyword("AND"), Some(Token::KeywordAnd));
        assert_eq!(Token::from_keyword("invalid"), None);
    }

    #[test]
    fn test_check_keyword() {
        assert_eq!(check_keyword("and"), Token::KeywordAnd);
        assert_eq!(check_keyword("identifier"), Token::Identifier);
        assert_eq!(check_keyword("test*"), Token::IdentifierWithWildcard);
        assert_eq!(check_keyword(""), Token::Nil);
    }

    #[test]
    fn test_valid_token_sequence() {
        assert!(valid_token_sequence(Token::Nil, Token::StmtOneOf));
        assert!(valid_token_sequence(Token::KeywordAnd, Token::Identifier));
        assert!(!valid_token_sequence(Token::Identifier, Token::Identifier));
    }
}
use crate::lexer::token::{Token, Item};
use thiserror::Error;

/// Parse error types for the Sigma parser
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ParseError {
    /// Token type is not supported in this context
    #[error("unsupported token: {msg}")]
    UnsupportedToken { 
        /// Description of the unsupported token
        msg: String 
    },

    /// Invalid sequence of tokens found
    #[error("invalid token sequence: {prev:?} -> {next:?}")]
    InvalidTokenSequence {
        /// Previous token in the sequence
        prev: Item,
        /// Next token that caused the invalid sequence
        next: Item,
        /// All tokens collected so far
        collected: Vec<Item>,
    },

    /// Referenced condition item is missing
    #[error("missing condition item: {key}")]
    MissingConditionItem { 
        /// Key of the missing condition item
        key: String 
    },

    /// Token sequence is incomplete
    #[error("incomplete token sequence in expression '{expression}', last token: {last:?}")]
    IncompleteTokenSequence {
        /// Expression being parsed
        expression: String,
        /// Items collected so far
        items: Vec<Item>,
        /// Last token received
        last: Item,
    },

    /// Detection field is missing from rule
    #[error("missing detection field")]
    MissingDetection,

    /// Condition is missing from detection section
    #[error("missing condition in detection")]
    MissingCondition,

    /// Lexer has not been initialized
    #[error("lexer not initialized")]
    LexerNotInitialized,

    /// General parser error
    #[error("parser error: {0}")]
    ParserError(String),

    /// Wildcard pattern compilation failed
    #[error("wildcard compilation failed: {0}")]
    WildcardCompilationError(String),
    
    /// Invalid wildcard identifier format
    #[error("invalid wildcard identifier")]
    InvalidWildcardIdent,
    
    /// Parentheses are not properly matched
    #[error("unmatched parenthesis")]
    UnmatchedParenthesis,
    
    /// No matching wildcard pattern found
    #[error("no matching wildcard")]
    NoMatchingWildcard,
    
    /// Token collection limit exceeded
    #[error("token collection limit exceeded: {current} tokens, limit: {limit}")]
    TokenLimitExceeded {
        /// Current number of tokens collected
        current: usize,
        /// Maximum allowed tokens
        limit: usize,
    },
    
    /// Keyword construct is invalid
    #[error("invalid keyword construct")]
    InvalidKeywordConstruct,
    
    /// Selection construct is invalid
    #[error("invalid selection construct")]
    InvalidSelectionConstruct,
    
    /// Token was not expected in this context
    #[error("unexpected token: {token:?}")]
    UnexpectedToken { 
        /// The unexpected token
        token: Token 
    },
    
    /// Glob pattern is invalid
    #[error("invalid glob pattern: {pattern}, error: {error}")]
    InvalidGlobPattern { 
        /// The invalid pattern
        pattern: String, 
        /// Error description
        error: String 
    },
    
    /// Value type is not supported
    #[error("unsupported value type: {value_type}")]
    UnsupportedValueType { 
        /// The unsupported value type
        value_type: String 
    },
}

impl ParseError {
    /// Create an unsupported token error
    pub fn unsupported_token(msg: impl Into<String>) -> Self {
        Self::UnsupportedToken { msg: msg.into() }
    }

    /// Create an invalid sequence error
    pub fn invalid_sequence(prev: Item, next: Item, collected: Vec<Item>) -> Self {
        Self::InvalidTokenSequence {
            prev,
            next,
            collected,
        }
    }

    /// Create a missing condition item error
    pub fn missing_condition_item(key: impl Into<String>) -> Self {
        Self::MissingConditionItem { key: key.into() }
    }

    /// Create an incomplete sequence error
    pub fn incomplete_sequence(expression: String, items: Vec<Item>, last: Item) -> Self {
        Self::IncompleteTokenSequence {
            expression,
            items,
            last,
        }
    }

    /// Create a general parser error
    pub fn parser_error(msg: impl Into<String>) -> Self {
        Self::ParserError(msg.into())
    }
}

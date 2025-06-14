use crate::lexer::error::LexError;
use crate::lexer::token::{Item, Token};
use std::sync::Arc;
use thiserror::Error;

/// Parse error types for the Sigma parser
#[derive(Debug, Error, Clone)]
pub enum ParseError {
    /// Token type is not supported in this context
    #[error("unsupported token: {msg}")]
    UnsupportedToken {
        /// Description of the unsupported token
        msg: String,
    },

    /// Invalid sequence of tokens found
    #[error("invalid token sequence: {prev:?} -> {next:?} (position: {position}, total tokens: {token_count})")]
    InvalidTokenSequence {
        /// Previous token in the sequence
        prev: Item,
        /// Next token that caused the invalid sequence
        next: Item,
        /// Position where error occurred
        position: usize,
        /// Total tokens collected so far
        token_count: usize,
        /// Last few tokens for context (up to 5)
        context_tokens: Box<Vec<Item>>,
    },

    /// Referenced condition item is missing
    #[error("missing condition item: {key}")]
    MissingConditionItem {
        /// Key of the missing condition item
        key: String,
    },

    /// Token sequence is incomplete
    #[error("incomplete token sequence in expression '{expression}', last token: {last:?}")]
    IncompleteTokenSequence {
        /// Expression being parsed
        expression: String,
        /// Items collected so far
        items: Box<Vec<Item>>,
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
        token: Token,
    },

    /// Glob pattern is invalid
    #[error("invalid glob pattern: {pattern}, error: {error}")]
    InvalidGlobPattern {
        /// The invalid pattern
        pattern: String,
        /// Error description
        error: String,
    },

    /// Value type is not supported
    #[error("unsupported value type: {value_type}")]
    UnsupportedValueType {
        /// The unsupported value type
        value_type: String,
    },

    /// Recursion depth limit exceeded during parsing
    #[error("recursion depth limit exceeded: {current} levels, limit: {limit}")]
    RecursionLimitExceeded {
        /// Current recursion depth
        current: usize,
        /// Maximum allowed depth
        limit: usize,
    },

    /// Invalid branch structure encountered
    #[error("invalid branch structure: {message}")]
    InvalidBranchStructure {
        /// Description of the branch structure error
        message: String,
    },

    /// Field pattern creation failed
    #[error("failed to create pattern for field '{field}': {error}")]
    FieldPatternCreationFailed {
        /// Field name
        field: String,
        /// Pattern value attempted
        value: String,
        /// Underlying error
        error: String,
    },

    /// Numeric pattern creation failed
    #[error("failed to create numeric pattern for value '{value}' in field '{field}': {error}")]
    NumericPatternCreationFailed {
        /// Field name
        field: String,
        /// Numeric value attempted
        value: String,
        /// Underlying error
        error: String,
    },

    /// String pattern creation failed
    #[error("failed to create string pattern for value '{value}' in field '{field}': {error}")]
    StringPatternCreationFailed {
        /// Field name
        field: String,
        /// String value attempted
        value: String,
        /// Underlying error
        error: String,
    },

    /// Detection parsing failed
    #[error("failed to parse detection section in rule '{rule_id}': {error}")]
    DetectionParsingFailed {
        /// Rule ID being parsed
        rule_id: String,
        /// Underlying error
        error: String,
    },

    /// No valid field patterns found
    #[error("no valid field patterns found in rule '{rule_id}', field '{field}': {errors:?}")]
    NoValidFieldPatterns {
        /// Rule ID being parsed
        rule_id: String,
        /// Field name
        field: String,
        /// Collection of errors encountered
        errors: Vec<String>,
    },

    /// Memory limit exceeded during token collection
    #[error("memory limit exceeded: {current_bytes} bytes used, limit: {limit_bytes} bytes")]
    MemoryLimitExceeded {
        /// Current memory usage in bytes
        current_bytes: usize,
        /// Memory limit in bytes
        limit_bytes: usize,
    },

    /// Task join error
    #[error("task join error: {0}")]
    TaskJoinError(String),

    /// Lexer error propagation
    #[error("lexer error: {0}")]
    LexerError(Arc<LexError>),
}

impl PartialEq for ParseError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::UnsupportedToken { msg: l }, Self::UnsupportedToken { msg: r }) => l == r,
            (
                Self::InvalidTokenSequence {
                    prev: l_prev,
                    next: l_next,
                    position: l_pos,
                    token_count: l_count,
                    context_tokens: l_ctx,
                },
                Self::InvalidTokenSequence {
                    prev: r_prev,
                    next: r_next,
                    position: r_pos,
                    token_count: r_count,
                    context_tokens: r_ctx,
                },
            ) => {
                l_prev == r_prev
                    && l_next == r_next
                    && l_pos == r_pos
                    && l_count == r_count
                    && l_ctx == r_ctx
            }
            (Self::MissingConditionItem { key: l }, Self::MissingConditionItem { key: r }) => {
                l == r
            }
            (
                Self::IncompleteTokenSequence {
                    expression: l_expr,
                    items: l_items,
                    last: l_last,
                },
                Self::IncompleteTokenSequence {
                    expression: r_expr,
                    items: r_items,
                    last: r_last,
                },
            ) => l_expr == r_expr && l_items == r_items && l_last == r_last,
            (Self::MissingDetection, Self::MissingDetection) => true,
            (Self::MissingCondition, Self::MissingCondition) => true,
            (Self::LexerNotInitialized, Self::LexerNotInitialized) => true,
            (Self::ParserError(l), Self::ParserError(r)) => l == r,
            (Self::WildcardCompilationError(l), Self::WildcardCompilationError(r)) => l == r,
            (Self::InvalidWildcardIdent, Self::InvalidWildcardIdent) => true,
            (Self::UnmatchedParenthesis, Self::UnmatchedParenthesis) => true,
            (Self::NoMatchingWildcard, Self::NoMatchingWildcard) => true,
            (
                Self::TokenLimitExceeded {
                    current: l_curr,
                    limit: l_lim,
                },
                Self::TokenLimitExceeded {
                    current: r_curr,
                    limit: r_lim,
                },
            ) => l_curr == r_curr && l_lim == r_lim,
            (Self::InvalidKeywordConstruct, Self::InvalidKeywordConstruct) => true,
            (Self::InvalidSelectionConstruct, Self::InvalidSelectionConstruct) => true,
            (Self::UnexpectedToken { token: l }, Self::UnexpectedToken { token: r }) => l == r,
            (
                Self::InvalidGlobPattern {
                    pattern: l_pat,
                    error: l_err,
                },
                Self::InvalidGlobPattern {
                    pattern: r_pat,
                    error: r_err,
                },
            ) => l_pat == r_pat && l_err == r_err,
            (
                Self::UnsupportedValueType { value_type: l },
                Self::UnsupportedValueType { value_type: r },
            ) => l == r,
            (
                Self::RecursionLimitExceeded {
                    current: l_curr,
                    limit: l_lim,
                },
                Self::RecursionLimitExceeded {
                    current: r_curr,
                    limit: r_lim,
                },
            ) => l_curr == r_curr && l_lim == r_lim,
            (
                Self::InvalidBranchStructure { message: l },
                Self::InvalidBranchStructure { message: r },
            ) => l == r,
            (
                Self::FieldPatternCreationFailed {
                    field: l_f,
                    value: l_v,
                    error: l_e,
                },
                Self::FieldPatternCreationFailed {
                    field: r_f,
                    value: r_v,
                    error: r_e,
                },
            ) => l_f == r_f && l_v == r_v && l_e == r_e,
            (
                Self::NumericPatternCreationFailed {
                    field: l_f,
                    value: l_v,
                    error: l_e,
                },
                Self::NumericPatternCreationFailed {
                    field: r_f,
                    value: r_v,
                    error: r_e,
                },
            ) => l_f == r_f && l_v == r_v && l_e == r_e,
            (
                Self::StringPatternCreationFailed {
                    field: l_f,
                    value: l_v,
                    error: l_e,
                },
                Self::StringPatternCreationFailed {
                    field: r_f,
                    value: r_v,
                    error: r_e,
                },
            ) => l_f == r_f && l_v == r_v && l_e == r_e,
            (
                Self::DetectionParsingFailed {
                    rule_id: l_id,
                    error: l_e,
                },
                Self::DetectionParsingFailed {
                    rule_id: r_id,
                    error: r_e,
                },
            ) => l_id == r_id && l_e == r_e,
            (
                Self::NoValidFieldPatterns {
                    rule_id: l_id,
                    field: l_f,
                    errors: l_e,
                },
                Self::NoValidFieldPatterns {
                    rule_id: r_id,
                    field: r_f,
                    errors: r_e,
                },
            ) => l_id == r_id && l_f == r_f && l_e == r_e,
            (
                Self::MemoryLimitExceeded {
                    current_bytes: l_c,
                    limit_bytes: l_l,
                },
                Self::MemoryLimitExceeded {
                    current_bytes: r_c,
                    limit_bytes: r_l,
                },
            ) => l_c == r_c && l_l == r_l,
            (Self::TaskJoinError(l), Self::TaskJoinError(r)) => l == r,
            (Self::LexerError(l), Self::LexerError(r)) => {
                // Compare by string representation since LexError might not implement PartialEq
                l.to_string() == r.to_string()
            }
            _ => false,
        }
    }
}

impl ParseError {
    /// Create an unsupported token error
    pub fn unsupported_token(msg: impl Into<String>) -> Self {
        Self::UnsupportedToken { msg: msg.into() }
    }

    /// Create an invalid sequence error
    pub fn invalid_sequence(prev: Item, next: Item, collected: &[Item]) -> Self {
        let token_count = collected.len();
        let position = token_count; // Position is at the end of collected tokens

        // Get last 5 tokens for context
        let context_start = token_count.saturating_sub(5);
        let context_tokens = collected[context_start..].to_vec();

        Self::InvalidTokenSequence {
            prev,
            next,
            position,
            token_count,
            context_tokens: Box::new(context_tokens),
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
            items: Box::new(items),
            last,
        }
    }

    /// Create a general parser error
    pub fn parser_error(msg: impl Into<String>) -> Self {
        Self::ParserError(msg.into())
    }

    /// Create a field pattern creation error
    pub fn field_pattern_creation_failed(
        field: impl Into<String>,
        value: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self::FieldPatternCreationFailed {
            field: field.into(),
            value: value.into(),
            error: error.into(),
        }
    }

    /// Create a numeric pattern creation error
    pub fn numeric_pattern_creation_failed(
        field: impl Into<String>,
        value: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self::NumericPatternCreationFailed {
            field: field.into(),
            value: value.into(),
            error: error.into(),
        }
    }

    /// Create a string pattern creation error
    pub fn string_pattern_creation_failed(
        field: impl Into<String>,
        value: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self::StringPatternCreationFailed {
            field: field.into(),
            value: value.into(),
            error: error.into(),
        }
    }

    /// Create a detection parsing error
    pub fn detection_parsing_failed(rule_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self::DetectionParsingFailed {
            rule_id: rule_id.into(),
            error: error.into(),
        }
    }

    /// Create a no valid field patterns error
    pub fn no_valid_field_patterns(
        rule_id: impl Into<String>,
        field: impl Into<String>,
        errors: Vec<String>,
    ) -> Self {
        Self::NoValidFieldPatterns {
            rule_id: rule_id.into(),
            field: field.into(),
            errors,
        }
    }
}

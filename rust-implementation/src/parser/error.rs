use crate::lexer::token::{Token, Item};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ParseError {
    #[error("unsupported token: {msg}")]
    UnsupportedToken { msg: String },

    #[error("invalid token sequence: {prev:?} -> {next:?}")]
    InvalidTokenSequence {
        prev: Item,
        next: Item,
        collected: Vec<Item>,
    },

    #[error("missing condition item: {key}")]
    MissingConditionItem { key: String },

    #[error("incomplete token sequence in expression '{expression}', last token: {last:?}")]
    IncompleteTokenSequence {
        expression: String,
        items: Vec<Item>,
        last: Item,
    },

    #[error("missing detection field")]
    MissingDetection,

    #[error("missing condition in detection")]
    MissingCondition,

    #[error("lexer not initialized")]
    LexerNotInitialized,

    #[error("parser error: {0}")]
    ParserError(String),

    #[error("wildcard compilation failed: {0}")]
    WildcardCompilationError(String),
}

impl ParseError {
    pub fn unsupported_token(msg: impl Into<String>) -> Self {
        Self::UnsupportedToken { msg: msg.into() }
    }

    pub fn invalid_sequence(prev: Item, next: Item, collected: Vec<Item>) -> Self {
        Self::InvalidTokenSequence {
            prev,
            next,
            collected,
        }
    }

    pub fn missing_condition_item(key: impl Into<String>) -> Self {
        Self::MissingConditionItem { key: key.into() }
    }

    pub fn incomplete_sequence(expression: String, items: Vec<Item>, last: Item) -> Self {
        Self::IncompleteTokenSequence {
            expression,
            items,
            last,
        }
    }

    pub fn parser_error(msg: impl Into<String>) -> Self {
        Self::ParserError(msg.into())
    }
}

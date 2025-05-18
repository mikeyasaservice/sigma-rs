use thiserror::Error;
use crate::lexer::token::Token;

/// Errors that can occur during lexing
#[derive(Debug, Error)]
pub enum LexError {
    #[error("Channel closed unexpectedly")]
    ChannelClosed,
    
    #[error("Unsupported token: {0}")]
    UnsupportedToken(String),
    
    #[error("Invalid token sequence: {prev:?} -> {next:?}")]
    InvalidSequence { prev: Token, next: Token },
    
    #[error("Unexpected end of input")]
    UnexpectedEof,
    
    #[error("Invalid UTF-8 sequence at position {0}")]
    InvalidUtf8(usize),
    
    #[error("Parse error: {0}")]
    ParseError(String),
}
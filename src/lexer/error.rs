use thiserror::Error;
use crate::lexer::token::Token;

/// Errors that can occur during lexing
#[derive(Debug, Error)]
pub enum LexError {
    #[error("Channel closed unexpectedly: {0}")]
    ChannelClosed(String),
    
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
    
    #[error("Lexing timeout exceeded")]
    Timeout,
    
    #[error("Position overflow: position would exceed maximum value")]
    PositionOverflow,
    
    #[error("Channel buffer full: receiver not keeping up with token production")]
    ChannelFull,
}
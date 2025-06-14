use crate::lexer::token::Token;
use thiserror::Error;

/// Errors that can occur during lexing
#[derive(Debug, Error)]
pub enum LexError {
    /// Channel was closed while trying to send data
    #[error("Channel closed unexpectedly: {0}")]
    ChannelClosed(String),

    /// Encountered an unsupported token type
    #[error("Unsupported token: {0}")]
    UnsupportedToken(String),

    /// Invalid token sequence detected
    #[error("Invalid token sequence: {prev:?} -> {next:?}")]
    InvalidSequence {
        /// Previous token in the sequence
        prev: Token,
        /// Next token that caused the error
        next: Token,
    },

    /// Unexpected end of file reached
    #[error("Unexpected end of input")]
    UnexpectedEof,

    /// Invalid UTF-8 sequence at position
    #[error("Invalid UTF-8 sequence at position {0}")]
    InvalidUtf8(usize),

    /// Generic parse error with message
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Lexing operation timed out
    #[error("Lexing timeout exceeded")]
    Timeout,

    /// Position counter overflowed
    #[error("Position overflow: position would exceed maximum value")]
    PositionOverflow,

    /// Token channel is full
    #[error("Channel buffer full: receiver not keeping up with token production")]
    ChannelFull,
}

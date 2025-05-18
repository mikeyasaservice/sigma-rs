//! Lexical analysis module

pub mod token;
pub mod state;
pub mod error;

use std::borrow::Cow;
use tokio::sync::mpsc;
use crate::lexer::token::{Token, Item};
use crate::lexer::state::LexState;
use crate::lexer::error::LexError;

/// Lexer for Sigma detection rules
/// Converts input string into a stream of tokens
pub struct Lexer {
    input: String,
    start: usize,
    position: usize,
    width: usize,
    items_tx: mpsc::UnboundedSender<Item>,
}

impl Lexer {
    /// Create a new lexer and return it with a token receiver
    pub fn new(input: String) -> (Self, mpsc::UnboundedReceiver<Item>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let lexer = Lexer {
            input,
            start: 0,
            position: 0,
            width: 0,
            items_tx: tx,
        };
        (lexer, rx)
    }

    /// Start scanning the input asynchronously
    pub async fn scan(mut self) -> Result<(), LexError> {
        let mut state = Some(LexState::Condition);
        while let Some(s) = state {
            state = self.process_state(s).await?;
        }
        Ok(())
    }

    /// Process the current state and return the next state
    async fn process_state(&mut self, state: LexState) -> Result<Option<LexState>, LexError> {
        match state {
            LexState::Condition => self.lex_condition().await,
            LexState::OneOf => self.lex_one_of().await,
            LexState::AllOf => self.lex_all_of().await,
            LexState::Eof => self.lex_eof().await,
            LexState::Pipe => self.lex_pipe().await,
            LexState::Lpar => self.lex_lpar().await,
            LexState::Rpar => self.lex_rpar().await,
            LexState::RparWithTokens => self.lex_rpar_with_tokens().await,
            LexState::AccumulateBeforeWhitespace => self.lex_accumulate_before_whitespace().await,
            LexState::Whitespace => self.lex_whitespace().await,
            LexState::Aggregation => self.lex_aggregation().await,
        }
    }

    /// Get the next character from the input
    fn next_char(&mut self) -> Option<char> {
        if self.position >= self.input.len() {
            self.width = 0;
            return None;
        }

        let remainder = &self.input[self.position..];
        if let Some(ch) = remainder.chars().next() {
            self.width = ch.len_utf8();
            self.position += self.width;
            Some(ch)
        } else {
            self.width = 0;
            None
        }
    }

    /// Back up one character
    fn backup(&mut self) {
        if self.position > 0 && self.width > 0 {
            self.position -= self.width;
            self.width = 0;
        }
    }

    /// Ignore characters up to current position
    fn ignore(&mut self) {
        self.start = self.position;
    }

    /// Get the collected string from start to current position
    fn collected(&self) -> &str {
        &self.input[self.start..self.position]
    }

    /// Get the remaining string from current position
    fn remaining(&self) -> &str {
        &self.input[self.position..]
    }

    /// Emit a token with the collected value
    async fn emit(&mut self, token: Token) -> Result<(), LexError> {
        let value = self.collected().to_string();
        let item = Item::new(token, value);
        self.items_tx.send(item).map_err(|_| LexError::ChannelClosed)?;
        self.ignore();
        Ok(())
    }

    /// Emit an error token
    async fn error(&mut self, msg: String) -> Result<Option<LexState>, LexError> {
        let item = Item::new(Token::Error, msg);
        self.items_tx.send(item).map_err(|_| LexError::ChannelClosed)?;
        Ok(None)
    }

    /// Emit an unsupported token
    async fn unsupported(&mut self, msg: String) -> Result<Option<LexState>, LexError> {
        let item = Item::new(Token::Unsupported, msg);
        self.items_tx.send(item).map_err(|_| LexError::ChannelClosed)?;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lexer_creation() {
        let (lexer, _rx) = Lexer::new("test".to_string());
        assert_eq!(lexer.position, 0);
        assert_eq!(lexer.start, 0);
        assert_eq!(lexer.input, "test");
    }
}

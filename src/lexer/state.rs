use crate::lexer::error::LexError;
use crate::lexer::{
    token::{check_keyword, Token},
    Lexer,
};

/// States in the lexer state machine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LexState {
    /// Main condition parsing state
    Condition,
    /// Parsing "1 of" statement
    OneOf,
    /// Parsing "all of" statement
    AllOf,
    /// End of file reached
    Eof,
    /// Pipe separator found
    Pipe,
    /// Left parenthesis found
    Lpar,
    /// Right parenthesis found
    Rpar,
    /// Right parenthesis with preceding tokens
    RparWithTokens,
    /// Accumulating tokens before whitespace
    AccumulateBeforeWhitespace,
    /// Processing whitespace
    Whitespace,
    /// Aggregation state (unsupported)
    Aggregation,
}

impl Lexer {
    /// Main lexing state - scans for tokens
    pub async fn lex_condition(&mut self) -> Result<Option<LexState>, LexError> {
        loop {
            if self.remaining().starts_with("1 of") {
                return Ok(Some(LexState::OneOf));
            }
            if self.remaining().starts_with("all of") {
                return Ok(Some(LexState::AllOf));
            }

            match self.next_char() {
                None => return Ok(Some(LexState::Eof)),
                Some(')') => return Ok(Some(LexState::RparWithTokens)),
                Some('(') => return Ok(Some(LexState::Lpar)),
                Some('|') => return Ok(Some(LexState::Pipe)),
                Some(c) if c.is_whitespace() => {
                    return Ok(Some(LexState::AccumulateBeforeWhitespace));
                }
                Some(_) => continue,
            }
        }
    }

    /// Lex "1 of" statement
    pub async fn lex_one_of(&mut self) -> Result<Option<LexState>, LexError> {
        self.position = self
            .position
            .checked_add("1 of".len())
            .ok_or(LexError::PositionOverflow)?;
        self.emit(Token::StmtOneOf).await?;
        Ok(Some(LexState::Condition))
    }

    /// Lex "all of" statement
    pub async fn lex_all_of(&mut self) -> Result<Option<LexState>, LexError> {
        self.position = self
            .position
            .checked_add("all of".len())
            .ok_or(LexError::PositionOverflow)?;
        self.emit(Token::StmtAllOf).await?;
        Ok(Some(LexState::Condition))
    }

    /// Lex end of file
    pub async fn lex_eof(&mut self) -> Result<Option<LexState>, LexError> {
        if self.position > self.start {
            let token = check_keyword(self.collected());
            self.emit(token).await?;
        }
        self.emit(Token::LitEof).await?;
        Ok(None)
    }

    /// Lex pipe separator
    pub async fn lex_pipe(&mut self) -> Result<Option<LexState>, LexError> {
        self.emit(Token::SepPipe).await?;
        Ok(Some(LexState::Aggregation))
    }

    /// Lex left parenthesis
    pub async fn lex_lpar(&mut self) -> Result<Option<LexState>, LexError> {
        self.emit(Token::SepLpar).await?;
        Ok(Some(LexState::Condition))
    }

    /// Lex right parenthesis with potential preceding tokens
    pub async fn lex_rpar_with_tokens(&mut self) -> Result<Option<LexState>, LexError> {
        if self.position > self.start {
            self.backup();
            let token = check_keyword(self.collected());
            if token != Token::Nil {
                self.emit(token).await?;
            }

            // Skip whitespace
            loop {
                match self.next_char() {
                    None => return Ok(Some(LexState::Eof)),
                    Some(c) if c.is_whitespace() => {
                        self.ignore();
                    }
                    Some(_) => {
                        return Ok(Some(LexState::Rpar));
                    }
                }
            }
        }
        Ok(Some(LexState::Rpar))
    }

    /// Lex right parenthesis
    pub async fn lex_rpar(&mut self) -> Result<Option<LexState>, LexError> {
        self.emit(Token::SepRpar).await?;
        Ok(Some(LexState::Condition))
    }

    /// Handle accumulated text before whitespace
    pub async fn lex_accumulate_before_whitespace(&mut self) -> Result<Option<LexState>, LexError> {
        self.backup();
        if self.position > self.start {
            let token = check_keyword(self.collected());
            self.emit(token).await?;
        }
        Ok(Some(LexState::Whitespace))
    }

    /// Lex whitespace
    pub async fn lex_whitespace(&mut self) -> Result<Option<LexState>, LexError> {
        loop {
            match self.next_char() {
                None => return Ok(Some(LexState::Eof)),
                Some(c) if !c.is_whitespace() => {
                    self.backup();
                    return Ok(Some(LexState::Condition));
                }
                Some(_) => {
                    self.ignore();
                }
            }
        }
    }

    /// Lex aggregation (currently unsupported)
    pub async fn lex_aggregation(&mut self) -> Result<Option<LexState>, LexError> {
        // Consume all remaining input
        self.position = self.input.len();
        self.emit(Token::Unsupported).await?;
        self.emit(Token::LitEof).await?;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn create_test_lexer(input: &str) -> (Lexer, mpsc::Receiver<crate::lexer::token::Item>) {
        Lexer::new(input)
    }

    #[tokio::test]
    async fn test_lex_one_of() {
        let (mut lexer, mut rx) = create_test_lexer("1 of");

        // Manually test the one_of state
        let next_state = lexer.lex_one_of().await.expect("lex_one_of should succeed");
        assert_eq!(next_state, Some(LexState::Condition));

        // Check the emitted token
        let item = rx.recv().await.expect("Should receive StmtOneOf token");
        assert_eq!(item.token, Token::StmtOneOf);
        assert_eq!(item.value, "1 of");
    }

    #[tokio::test]
    async fn test_lex_all_of() {
        let (mut lexer, mut rx) = create_test_lexer("all of");

        let next_state = lexer.lex_all_of().await.expect("lex_all_of should succeed");
        assert_eq!(next_state, Some(LexState::Condition));

        let item = rx.recv().await.expect("Should receive StmtAllOf token");
        assert_eq!(item.token, Token::StmtAllOf);
        assert_eq!(item.value, "all of");
    }
}

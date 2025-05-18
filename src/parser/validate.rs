use crate::lexer::token::Token;

/// Validates that two tokens can appear in sequence
pub fn valid_token_sequence(t1: Token, t2: Token) -> bool {
    match t2 {
        Token::StmtAll | Token::StmtOne => match t1 {
            Token::Begin
            | Token::SeparatorLeftParen
            | Token::KeywordAnd
            | Token::KeywordOr
            | Token::KeywordNot => true,
            _ => false,
        },
        Token::IdentifierAll => match t1 {
            Token::StmtAll | Token::StmtOne => true,
            _ => false,
        },
        Token::Identifier | Token::IdentifierWithWildcard => match t1 {
            Token::SeparatorLeftParen
            | Token::Begin
            | Token::KeywordAnd
            | Token::KeywordOr
            | Token::KeywordNot
            | Token::StmtOne
            | Token::StmtAll => true,
            _ => false,
        },
        Token::KeywordAnd | Token::KeywordOr => match t1 {
            Token::Identifier
            | Token::IdentifierAll
            | Token::IdentifierWithWildcard
            | Token::SeparatorRightParen => true,
            _ => false,
        },
        Token::KeywordNot => match t1 {
            Token::KeywordAnd
            | Token::KeywordOr
            | Token::SeparatorLeftParen
            | Token::Begin => true,
            _ => false,
        },
        Token::SeparatorLeftParen => match t1 {
            Token::KeywordAnd
            | Token::KeywordOr
            | Token::KeywordNot
            | Token::Begin
            | Token::SeparatorLeftParen => true,
            _ => false,
        },
        Token::SeparatorRightParen => match t1 {
            Token::Identifier
            | Token::IdentifierAll
            | Token::IdentifierWithWildcard
            | Token::SeparatorLeftParen
            | Token::SeparatorRightParen => true,
            _ => false,
        },
        Token::Eof => match t1 {
            Token::Identifier
            | Token::IdentifierAll
            | Token::IdentifierWithWildcard
            | Token::SeparatorRightParen => true,
            _ => false,
        },
        Token::SeparatorPipe => match t1 {
            Token::Identifier
            | Token::IdentifierAll
            | Token::IdentifierWithWildcard
            | Token::SeparatorRightParen => true,
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Token;

    #[test]
    fn test_valid_sequences() {
        // Test BEGIN token sequences
        assert!(valid_token_sequence(Token::Begin, Token::Identifier));
        assert!(valid_token_sequence(Token::Begin, Token::KeywordNot));
        assert!(valid_token_sequence(Token::Begin, Token::SeparatorLeftParen));
        assert!(valid_token_sequence(Token::Begin, Token::StmtAll));
        assert!(valid_token_sequence(Token::Begin, Token::StmtOne));

        // Test identifier sequences
        assert!(valid_token_sequence(
            Token::Identifier,
            Token::KeywordAnd
        ));
        assert!(valid_token_sequence(Token::Identifier, Token::KeywordOr));
        assert!(valid_token_sequence(Token::Identifier, Token::Eof));

        // Test keyword sequences
        assert!(valid_token_sequence(
            Token::KeywordAnd,
            Token::Identifier
        ));
        assert!(valid_token_sequence(
            Token::KeywordNot,
            Token::Identifier
        ));

        // Test statement sequences
        assert!(valid_token_sequence(
            Token::StmtAll,
            Token::IdentifierAll
        ));
        assert!(valid_token_sequence(Token::StmtOne, Token::Identifier));

        // Test parentheses
        assert!(valid_token_sequence(
            Token::SeparatorLeftParen,
            Token::Identifier
        ));
        assert!(valid_token_sequence(
            Token::Identifier,
            Token::SeparatorRightParen
        ));
    }

    #[test]
    fn test_invalid_sequences() {
        // Invalid sequences
        assert!(!valid_token_sequence(Token::Identifier, Token::Identifier));
        assert!(!valid_token_sequence(
            Token::KeywordAnd,
            Token::KeywordAnd
        ));
        assert!(!valid_token_sequence(
            Token::KeywordNot,
            Token::KeywordAnd
        ));
        assert!(!valid_token_sequence(Token::Begin, Token::Eof));
        assert!(!valid_token_sequence(
            Token::SeparatorRightParen,
            Token::SeparatorLeftParen
        ));
    }
}

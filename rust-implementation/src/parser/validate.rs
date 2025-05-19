use crate::lexer::token::Token;

/// Validates that two tokens can appear in sequence
pub fn valid_token_sequence(t1: Token, t2: Token) -> bool {
    // Special handling for begin state
    if is_begin_token(t1) {
        return matches!(t2, 
            Token::Identifier | 
            Token::IdentifierWithWildcard |
            Token::KeywordNot |
            Token::SepLpar |
            Token::StmtAllOf |
            Token::StmtOneOf
        );
    }
    
    match t2 {
        Token::StmtAllOf | Token::StmtOneOf => match t1 {
            Token::SepLpar
            | Token::KeywordAnd
            | Token::KeywordOr
            | Token::KeywordNot => true,
            _ => is_begin_token(t1),
        },
        Token::IdentifierAll => matches!(t1, Token::StmtAllOf | Token::StmtOneOf),
        Token::Identifier | Token::IdentifierWithWildcard => match t1 {
            Token::SepLpar
            | Token::KeywordAnd
            | Token::KeywordOr
            | Token::KeywordNot
            | Token::StmtOneOf
            | Token::StmtAllOf => true,
            _ => is_begin_token(t1),
        },
        Token::KeywordAnd | Token::KeywordOr => matches!(t1,
            Token::Identifier
            | Token::IdentifierAll
            | Token::IdentifierWithWildcard
            | Token::SepRpar
        ),
        Token::KeywordNot => match t1 {
            Token::KeywordAnd
            | Token::KeywordOr
            | Token::SepLpar => true,
            _ => is_begin_token(t1),
        },
        Token::SepLpar => match t1 {
            Token::KeywordAnd
            | Token::KeywordOr
            | Token::KeywordNot
            | Token::SepLpar => true,
            _ => is_begin_token(t1),
        },
        Token::SepRpar => matches!(t1,
            Token::Identifier
            | Token::IdentifierAll
            | Token::IdentifierWithWildcard
            | Token::SepLpar
            | Token::SepRpar
        ),
        Token::LitEof => matches!(t1,
            Token::Identifier
            | Token::IdentifierAll
            | Token::IdentifierWithWildcard
            | Token::SepRpar
        ),
        Token::SepPipe => matches!(t1,
            Token::Identifier
            | Token::IdentifierAll
            | Token::IdentifierWithWildcard
            | Token::SepRpar
        ),
        _ => false,
    }
}

/// Check if a token represents the begin state
/// We use Identifier with value "<begin>" as placeholder
fn is_begin_token(_token: Token) -> bool {
    // In practice we check this through the Item's value
    false // This will be checked in the parser
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Token;

    #[test]
    fn test_valid_sequences() {
        // Test identifier sequences
        assert!(valid_token_sequence(Token::Identifier, Token::KeywordAnd));
        assert!(valid_token_sequence(Token::Identifier, Token::KeywordOr));
        assert!(valid_token_sequence(Token::Identifier, Token::LitEof));

        // Test keyword sequences
        assert!(valid_token_sequence(Token::KeywordAnd, Token::Identifier));
        assert!(valid_token_sequence(Token::KeywordNot, Token::Identifier));

        // Test statement sequences
        assert!(valid_token_sequence(Token::StmtAllOf, Token::IdentifierAll));
        assert!(valid_token_sequence(Token::StmtOneOf, Token::Identifier));

        // Test parentheses
        assert!(valid_token_sequence(Token::SepLpar, Token::Identifier));
        assert!(valid_token_sequence(Token::Identifier, Token::SepRpar));
    }

    #[test] 
    fn test_invalid_sequences() {
        // Invalid sequences
        assert!(!valid_token_sequence(Token::Identifier, Token::Identifier));
        assert!(!valid_token_sequence(Token::KeywordAnd, Token::KeywordAnd));
        assert!(!valid_token_sequence(Token::KeywordNot, Token::KeywordAnd));
        assert!(!valid_token_sequence(Token::SepRpar, Token::SepLpar));
    }
}
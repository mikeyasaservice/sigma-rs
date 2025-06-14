use sigma_rs::lexer::{token::Token, Lexer};
use tokio::sync::mpsc;

struct LexTestCase {
    expr: &'static str,
    tokens: Vec<Token>,
}

#[tokio::test]
async fn test_lexer_cases() {
    let test_cases = vec![
        LexTestCase {
            expr: "selection",
            tokens: vec![Token::Identifier, Token::LitEof],
        },
        LexTestCase {
            expr: "selection_1 and not filter_0",
            tokens: vec![
                Token::Identifier,
                Token::KeywordAnd,
                Token::KeywordNot,
                Token::Identifier,
                Token::LitEof,
            ],
        },
        LexTestCase {
            expr: "((selection_1 and not filter_0) OR (keyword_0 and not filter1)) or idontcare",
            tokens: vec![
                Token::SepLpar,
                Token::SepLpar,
                Token::Identifier,
                Token::KeywordAnd,
                Token::KeywordNot,
                Token::Identifier,
                Token::SepRpar,
                Token::KeywordOr,
                Token::SepLpar,
                Token::Identifier,
                Token::KeywordAnd,
                Token::KeywordNot,
                Token::Identifier,
                Token::SepRpar,
                Token::SepRpar,
                Token::KeywordOr,
                Token::Identifier,
                Token::LitEof,
            ],
        },
        LexTestCase {
            expr: "all of selection* and not 1 of filter* | count() > 10",
            tokens: vec![
                Token::StmtAllOf,
                Token::IdentifierWithWildcard,
                Token::KeywordAnd,
                Token::KeywordNot,
                Token::StmtOneOf,
                Token::IdentifierWithWildcard,
                Token::SepPipe,
                Token::Unsupported,
                Token::LitEof,
            ],
        },
    ];

    for (index, case) in test_cases.iter().enumerate() {
        let (lexer, mut rx) = Lexer::new(case.expr.to_string());

        // Spawn the lexer to run asynchronously
        tokio::spawn(async move {
            lexer.scan().await.unwrap();
        });

        // Collect all tokens
        let mut tokens = Vec::new();
        while let Some(item) = rx.recv().await {
            tokens.push(item.token);
        }

        // Compare with expected tokens
        if tokens.len() != case.tokens.len() {
            tracing::error!("Test case {} failed: expression '{}'", index, case.expr);
            tracing::error!("Expected tokens: {:?}", case.tokens);
            tracing::error!("Actual tokens: {:?}", tokens);
            tracing::error!(
                "Expected {} tokens, got {}",
                case.tokens.len(),
                tokens.len()
            );
            panic!("Token count mismatch");
        }

        for (i, (actual, expected)) in tokens.iter().zip(case.tokens.iter()).enumerate() {
            assert_eq!(
                actual, expected,
                "Test case {} failed on token {}: expression '{}' expected {:?}, got {:?}",
                index, i, case.expr, expected, actual
            );
        }
    }
}

#[tokio::test]
async fn test_simple_identifier() {
    let (lexer, mut rx) = Lexer::new("selection".to_string());

    tokio::spawn(async move {
        lexer.scan().await.unwrap();
    });

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::Identifier);
    assert_eq!(item.value, "selection");

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::LitEof);
}

#[tokio::test]
async fn test_keywords() {
    let cases = vec![
        ("and", Token::KeywordAnd),
        ("AND", Token::KeywordAnd),
        ("or", Token::KeywordOr),
        ("OR", Token::KeywordOr),
        ("not", Token::KeywordNot),
        ("NOT", Token::KeywordNot),
    ];

    for (input, expected) in cases {
        let (lexer, mut rx) = Lexer::new(input.to_string());

        tokio::spawn(async move {
            lexer.scan().await.unwrap();
        });

        let item = rx.recv().await.unwrap();
        assert_eq!(item.token, expected, "Failed to lex keyword: {}", input);
    }
}

#[tokio::test]
async fn test_statements() {
    let cases = vec![("1 of", Token::StmtOneOf), ("all of", Token::StmtAllOf)];

    for (input, expected) in cases {
        let (lexer, mut rx) = Lexer::new(input.to_string());

        tokio::spawn(async move {
            lexer.scan().await.unwrap();
        });

        let item = rx.recv().await.unwrap();
        assert_eq!(item.token, expected, "Failed to lex statement: {}", input);
    }
}

#[tokio::test]
async fn test_wildcard_identifiers() {
    let (lexer, mut rx) = Lexer::new("selection*".to_string());

    tokio::spawn(async move {
        lexer.scan().await.unwrap();
    });

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::IdentifierWithWildcard);
    assert_eq!(item.value, "selection*");
}

#[tokio::test]
async fn test_parentheses() {
    let (lexer, mut rx) = Lexer::new("(selection)".to_string());

    tokio::spawn(async move {
        lexer.scan().await.unwrap();
    });

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::SepLpar);

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::Identifier);
    assert_eq!(item.value, "selection");

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::SepRpar);

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::LitEof);
}

#[tokio::test]
async fn test_pipe_aggregation() {
    let (lexer, mut rx) = Lexer::new("selection | count() > 10".to_string());

    tokio::spawn(async move {
        lexer.scan().await.unwrap();
    });

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::Identifier);
    assert_eq!(item.value, "selection");

    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::SepPipe);

    // The rest should be unsupported for now
    let item = rx.recv().await.unwrap();
    assert_eq!(item.token, Token::Unsupported);
}

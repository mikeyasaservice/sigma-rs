use sigma_rs::lexer::Lexer;
use sigma_rs::lexer::token::Token;

#[tokio::test]
async fn debug_lexer_tokens() {
    let condition = "selection and not exclusion";
    let (lexer, mut rx) = Lexer::new(condition.to_string());
    
    tokio::spawn(async move {
        lexer.scan().await.unwrap();
    });
    
    println!("Tokens for: '{}'", condition);
    while let Some(item) = rx.recv().await {
        println!("Token: {:?}, Value: '{}'", item.token, item.value);
        if item.token == Token::LitEof {
            break;
        }
    }
}
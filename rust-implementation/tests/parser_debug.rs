use sigma_rs::lexer::Lexer;
use sigma_rs::parser::{Parser, ParseError};
use sigma_rs::detection::Detection;
use serde_json::json;

#[tokio::test]
async fn debug_parser_error() {
    let condition = "selection and not exclusion";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection".to_string(), json!({
        "EventID": 4688
    }));
    detection.insert("exclusion".to_string(), json!({
        "User": "SYSTEM"
    }));

    let (lexer, mut rx) = Lexer::new(condition.to_string());
    
    // Collect tokens manually
    tokio::spawn(async move {
        lexer.scan().await.unwrap();
    });
    
    println!("Collecting tokens...");
    let mut tokens = vec![];
    while let Some(item) = rx.recv().await {
        println!("Received token: {:?}, Value: '{}'", item.token, item.value);
        tokens.push(item);
    }
    
    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    if let Err(e) = &result {
        println!("Parser error: {:?}", e);
        match e {
            ParseError::InvalidTokenSequence { prev, next, collected } => {
                println!("Previous token: {:?}, Value: '{}'", prev.token, prev.value);
                println!("Next token: {:?}, Value: '{}'", next.token, next.value);
                println!("Collected so far: {:?}", collected.iter().map(|i| (&i.token, &i.value)).collect::<Vec<_>>());
            }
            _ => {}
        }
    }
}
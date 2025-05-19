use sigma_rs::parser::{Parser, ParseError};
use sigma_rs::rule::Detection;
use serde_json::json;

#[tokio::test]
async fn debug_parser_detailed() {
    let condition = "selection and not exclusion";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection".to_string(), json!({
        "EventID": 4688
    }));
    detection.insert("exclusion".to_string(), json!({
        "User": "SYSTEM"
    }));

    println!("Detection data:");
    println!("Condition: {:?}", condition);
    println!("Detection entries:");
    for (key, value) in detection.extract() {
        println!("  {}: {:?}", key, value);
    }
    
    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    match &result {
        Ok(_) => println!("Parser succeeded"),
        Err(e) => println!("Parser error: {:?}", e),
    }
}
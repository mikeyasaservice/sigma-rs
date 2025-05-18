use sigma_rs::parser::{Parser, ParseError};
use sigma_rs::detection::Detection;
use serde_json::json;

#[tokio::test]
async fn debug_all_of_statement() {
    let condition = "all of selection*";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection1".to_string(), json!({
        "EventID": 4688
    }));
    detection.insert("selection2".to_string(), json!({
        "Channel": "Security"
    }));
    detection.insert("selection_test".to_string(), json!({
        "Level": 4
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
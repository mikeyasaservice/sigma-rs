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

    tracing::error!("Detection data:");
    tracing::error!("Condition: {:?}", condition);
    tracing::error!("Detection entries:");
    for (key, value) in detection.extract() {
        tracing::error!("  {}: {:?}", key, value);
    }
    
    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    match &result {
        Ok(_) => tracing::error!("Parser succeeded"),
        Err(e) => tracing::error!("Parser error: {:?}", e),
    }
}
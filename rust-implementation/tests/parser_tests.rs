use sigma_rs::detection::Detection;
use sigma_rs::parser::{Parser, ParseError};
use std::collections::HashMap;
use serde_json::json;

#[tokio::test]
async fn test_simple_and_condition() {
    let condition = "selection1 and selection2";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection1".to_string(), json!({
        "EventID": 4688,
        "Channel": "Security"
    }));
    detection.insert("selection2".to_string(), json!({
        "CommandLine|contains": "mimikatz"
    }));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_simple_or_condition() {
    let condition = "selection1 or selection2";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection1".to_string(), json!({
        "Image|endswith": "\\cmd.exe"
    }));
    detection.insert("selection2".to_string(), json!({
        "Image|endswith": "\\powershell.exe" 
    }));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_not_condition() {
    let condition = "selection and not exclusion";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection".to_string(), json!({
        "EventID": 4688
    }));
    detection.insert("exclusion".to_string(), json!({
        "User": "SYSTEM"
    }));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_complex_parentheses() {
    let condition = "(selection1 or selection2) and not (exclusion1 or exclusion2)";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection1".to_string(), json!({
        "EventID": 4688
    }));
    detection.insert("selection2".to_string(), json!({
        "EventID": 4689
    }));
    detection.insert("exclusion1".to_string(), json!({
        "User": "SYSTEM"
    }));
    detection.insert("exclusion2".to_string(), json!({
        "User": "LOCAL SERVICE"
    }));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_all_of_statement() {
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

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_one_of_statement() {
    let condition = "1 of selection*";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection_cmd".to_string(), json!({
        "Image|endswith": "\\cmd.exe"
    }));
    detection.insert("selection_ps".to_string(), json!({
        "Image|endswith": "\\powershell.exe"
    }));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_all_of_them() {
    let condition = "all of them";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection1".to_string(), json!({
        "EventID": 4688
    }));
    detection.insert("selection2".to_string(), json!({
        "Channel": "Security"
    }));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_array_values() {
    let condition = "selection";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection".to_string(), json!({
        "Image": [
            "*\\cmd.exe",
            "*\\powershell.exe",
            "*\\pwsh.exe"
        ]
    }));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_nested_complex_condition() {
    let condition = "((proc and (all of selection*)) or (net and 1 of them)) and not exclusion";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("proc".to_string(), json!({
        "EventID": 1
    }));
    detection.insert("selection1".to_string(), json!({
        "Image|contains": "mimikatz"
    }));
    detection.insert("selection2".to_string(), json!({
        "CommandLine|contains": "sekurlsa"
    }));
    detection.insert("net".to_string(), json!({
        "EventID": 3
    }));
    detection.insert("exclusion".to_string(), json!({
        "User": "SYSTEM"
    }));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_ok());
    assert!(parser.result().is_some());
}

#[tokio::test]
async fn test_invalid_token_sequence() {
    let condition = "selection1 selection2"; // Invalid: two identifiers without operator
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection1".to_string(), json!("value1"));
    detection.insert("selection2".to_string(), json!("value2"));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(matches!(result, Err(ParseError::InvalidTokenSequence { .. })));
}

#[tokio::test]
async fn test_missing_selection() {
    let condition = "selection1 and selection2";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection1".to_string(), json!("value1"));
    // selection2 is missing

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(matches!(result, Err(ParseError::MissingConditionItem { .. })));
}

#[tokio::test]
async fn test_unbalanced_parentheses() {
    let condition = "(selection1 and selection2";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection1".to_string(), json!("value1"));
    detection.insert("selection2".to_string(), json!("value2"));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    // Should fail during collection phase due to incomplete sequence
    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_condition() {
    let condition = "";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));

    let mut parser = Parser::new(detection, false);
    
    let result = parser.run().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_whitespace_handling() {
    let condition = "selection";
    let mut detection = Detection::new();
    detection.insert("condition".to_string(), json!(condition));
    detection.insert("selection".to_string(), json!({
        "CommandLine": "test   multiple   spaces"
    }));

    // Test with whitespace collapse
    let mut parser1 = Parser::new(detection.clone(), false);
    let result1 = parser1.run().await;
    assert!(result1.is_ok());

    // Test without whitespace collapse
    let mut parser2 = Parser::new(detection, true);
    let result2 = parser2.run().await;
    assert!(result2.is_ok());
}
// Test file to debug escape behavior

use crate::pattern::escape::escape_sigma_for_glob;

#[test]
fn debug_escape() {
    let input = "test\\[abc\\]";
    let result = escape_sigma_for_glob(input);
    tracing::error!("Input:  {:?}", input);
    tracing::error!("Output: {:?}", result);
    
    // Check each character
    let input_bytes = input.as_bytes();
    let result_bytes = result.as_bytes();
    
    tracing::error!("\nInput bytes:");
    for (i, b) in input_bytes.iter().enumerate() {
        tracing::error!("{}: {} ({})", i, b, *b as char);
    }
    
    tracing::error!("\nResult bytes:");
    for (i, b) in result_bytes.iter().enumerate() {
        tracing::error!("{}: {} ({})", i, b, *b as char);
    }
}
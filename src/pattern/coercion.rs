//! Type coercion for Sigma pattern matching
//!
//! Implements type coercion for numeric values and string conversions
//! to match the behavior of the Go implementation.

use serde_json::Value;
use std::borrow::Cow;

/// Trait for types that can be coerced to match against patterns
pub trait Coercible {
    /// Convert to string representation for string matching
    fn to_string_match(&self) -> String;
    
    /// Convert to integer for numeric matching
    fn to_int_match(&self) -> Option<i64>;
    
    /// Convert to float for numeric matching
    fn to_float_match(&self) -> Option<f64>;
}

impl Coercible for Value {
    fn to_string_match(&self) -> String {
        match self {
            Value::String(s) => s.to_string(),
            Value::Number(n) => {
                // Handle JSON numbers similar to Go implementation
                if let Some(i) = n.as_i64() {
                    i.to_string()
                } else if let Some(u) = n.as_u64() {
                    u.to_string()
                } else if let Some(f) = n.as_f64() {
                    // For floats, convert to int for string matching (matching Go behavior)
                    (f as i64).to_string()
                } else {
                    n.to_string()
                }
            }
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            _ => self.to_string(),
        }
    }
    
    fn to_int_match(&self) -> Option<i64> {
        match self {
            Value::Number(n) => {
                n.as_i64()
                    .or_else(|| n.as_u64().and_then(|u| {
                        if u <= i64::MAX as u64 {
                            Some(u as i64)
                        } else {
                            None  // Value too large for i64
                        }
                    }))
                    .or_else(|| n.as_f64().and_then(|f| {
                        // Check if float is within i64 bounds
                        if f >= i64::MIN as f64 && f <= i64::MAX as f64 && f.is_finite() {
                            Some(f as i64)
                        } else {
                            None  // Value out of range or not finite
                        }
                    }))
            }
            Value::String(s) => s.parse::<i64>().ok(),
            _ => None,
        }
    }
    
    fn to_float_match(&self) -> Option<f64> {
        match self {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse::<f64>().ok(),
            _ => None,
        }
    }
}

/// Coerce a value for string pattern matching with copy-on-write semantics
pub fn coerce_for_string_match(value: &Value) -> Cow<str> {
    match value {
        Value::String(s) => Cow::Borrowed(s),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Cow::Owned(i.to_string())
            } else if let Some(u) = n.as_u64() {
                Cow::Owned(u.to_string())
            } else if let Some(f) = n.as_f64() {
                Cow::Owned((f as i64).to_string())
            } else {
                Cow::Owned(n.to_string())
            }
        }
        Value::Bool(b) => Cow::Owned(b.to_string()),
        Value::Null => Cow::Borrowed("null"),
        _ => Cow::Owned(format!("{:?}", value)),
    }
}

/// Coerce a value for string pattern matching (legacy function - returns String)
pub fn coerce_for_string_match_owned(value: &Value) -> String {
    coerce_for_string_match(value).into_owned()
}

/// Coerce a value for numeric pattern matching
/// Returns None if the value cannot be safely converted to i64
pub fn coerce_for_numeric_match(value: &Value) -> Option<i64> {
    value.to_int_match()
}

/// Check if a value can be coerced to a number
pub fn can_coerce_to_number(value: &Value) -> bool {
    matches!(value, Value::Number(_)) || 
    (match value {
        Value::String(s) => s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok(),
        _ => false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_string_coercion() {
        assert_eq!(coerce_for_string_match(&json!("test")), "test");
        assert_eq!(coerce_for_string_match(&json!(123)), "123");
        assert_eq!(coerce_for_string_match(&json!(123.456)), "123");
        assert_eq!(coerce_for_string_match(&json!(true)), "true");
        assert_eq!(coerce_for_string_match(&json!(null)), "null");
    }
    
    #[test]
    fn test_numeric_coercion() {
        assert_eq!(coerce_for_numeric_match(&json!(123)), Some(123));
        assert_eq!(coerce_for_numeric_match(&json!(123.456)), Some(123));
        assert_eq!(coerce_for_numeric_match(&json!("123")), Some(123));
        assert_eq!(coerce_for_numeric_match(&json!("not a number")), None);
        assert_eq!(coerce_for_numeric_match(&json!(true)), None);
    }
    
    #[test]
    fn test_can_coerce_to_number() {
        assert!(can_coerce_to_number(&json!(123)));
        assert!(can_coerce_to_number(&json!(123.456)));
        assert!(can_coerce_to_number(&json!("123")));
        assert!(can_coerce_to_number(&json!("123.456")));
        assert!(!can_coerce_to_number(&json!("not a number")));
        assert!(!can_coerce_to_number(&json!(true)));
        assert!(!can_coerce_to_number(&json!(null)));
    }
    
    #[test]
    fn test_large_numbers() {
        let large_u64 = u64::MAX;
        assert_eq!(coerce_for_string_match(&json!(large_u64)), large_u64.to_string());
        
        // Test u64 that fits in i64
        let fits_in_i64 = json!(9223372036854775807u64); // i64::MAX
        assert_eq!(coerce_for_numeric_match(&fits_in_i64), Some(i64::MAX));
        
        // Test u64 that overflows i64
        let large_num = json!(9223372036854775808u64); // i64::MAX + 1
        assert_eq!(coerce_for_numeric_match(&large_num), None);
        
        // Test u64::MAX
        assert_eq!(coerce_for_numeric_match(&json!(u64::MAX)), None);
    }
    
    #[test]
    fn test_float_to_int_bounds() {
        // Test float within bounds
        assert_eq!(coerce_for_numeric_match(&json!(123.456)), Some(123));
        
        // Test float at i64::MAX boundary
        let max_safe = json!(9223372036854775807.0);
        assert_eq!(coerce_for_numeric_match(&max_safe), Some(i64::MAX));
        
        // Test float beyond i64::MAX
        let too_large = json!(9223372036854775808.0);
        assert_eq!(coerce_for_numeric_match(&too_large), None);
        
        // Test float at i64::MIN boundary
        let min_safe = json!(-9223372036854775808.0);
        assert_eq!(coerce_for_numeric_match(&min_safe), Some(i64::MIN));
        
        // Test float beyond i64::MIN
        let too_small = json!(-9223372036854775809.0);
        assert_eq!(coerce_for_numeric_match(&too_small), None);
        
        // Test special float values
        assert_eq!(coerce_for_numeric_match(&json!(f64::INFINITY)), None);
        assert_eq!(coerce_for_numeric_match(&json!(f64::NEG_INFINITY)), None);
        assert_eq!(coerce_for_numeric_match(&json!(f64::NAN)), None);
    }
}
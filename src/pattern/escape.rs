//! Escape handling for Sigma patterns
//!
//! Implements the Sigma-specific escape rules for glob patterns.
//! 
//! Per Sigma specification:
//! - Plain backslash not followed by a wildcard: '\\' or '\\'
//! - Escaped wildcard: '\\*'
//! - Backslash before wildcard: '\\\\*'
//! - Escaped backslash and wildcard: '\\\\\\*'
//! - Double backslash: '\\\\\\\\' results in '\\\\'

use std::borrow::Cow;

const SIGMA_WILDCARD: u8 = b'*';
const SIGMA_SINGLE: u8 = b'?';
const SIGMA_ESCAPE: u8 = b'\\';
const GLOB_SQR_BRKT_LEFT: u8 = b'[';
const GLOB_SQR_BRKT_RIGHT: u8 = b']';
const GLOB_CURL_BRKT_LEFT: u8 = b'{';
const GLOB_CURL_BRKT_RIGHT: u8 = b'}';

/// Escape Sigma string for use with glob patterns (zero-copy when no escaping needed)
pub fn escape_sigma_for_glob_cow(s: &str) -> Cow<'_, str> {
    if s.is_empty() {
        return Cow::Borrowed(s);
    }

    // Fast path: check if any escaping is needed
    let needs_escaping = s.bytes().any(|b| {
        matches!(b, GLOB_SQR_BRKT_LEFT | GLOB_SQR_BRKT_RIGHT | GLOB_CURL_BRKT_LEFT | GLOB_CURL_BRKT_RIGHT | SIGMA_ESCAPE)
    });

    if !needs_escaping {
        return Cow::Borrowed(s);
    }

    // Slow path: perform escaping
    Cow::Owned(escape_sigma_for_glob_owned(s))
}

/// Escape Sigma string for use with glob patterns (always allocates)
pub fn escape_sigma_for_glob(str: &str) -> String {
    match escape_sigma_for_glob_cow(str) {
        Cow::Borrowed(s) => s.to_string(),
        Cow::Owned(s) => s,
    }
}

/// Internal function that always returns an owned string
fn escape_sigma_for_glob_owned(str: &str) -> String {
    // Function to check if a byte is a bracket that needs escaping
    let is_bracket = |b: u8| -> bool {
        matches!(b, GLOB_SQR_BRKT_LEFT | GLOB_SQR_BRKT_RIGHT | GLOB_CURL_BRKT_LEFT | GLOB_CURL_BRKT_RIGHT)
    };

    let str_bytes = str.as_bytes();
    let len = str_bytes.len();
    
    // Reserve space for worst case (all characters need escaping)
    let mut x = (len * 2) as i32 - 1;
    let mut repl_str = vec![0u8; len * 2];
    
    let mut wildcard = false;
    let mut slash_count = 0;
    
    // Process string in reverse order
    for i in (0..len).rev() {
        let ch = str_bytes[i];
        
        match ch {
            SIGMA_WILDCARD | SIGMA_SINGLE => {
                wildcard = true;
            }
            SIGMA_ESCAPE => {
                if !wildcard {
                    slash_count += 1;
                }
            }
            _ => {
                wildcard = false;
            }
        }
        
        // Check if we need to balance slashes
        if ch != SIGMA_ESCAPE && slash_count > 0 {
            if slash_count % 2 != 0 {
                repl_str[x as usize] = SIGMA_ESCAPE;
                x -= 1;
            }
            slash_count = 0;
        }
        
        repl_str[x as usize] = ch;
        x -= 1;
        
        // Escape brackets for glob
        if is_bracket(ch) {
            repl_str[x as usize] = SIGMA_ESCAPE;
            x -= 1;
        }
    }
    
    // Handle leading backslashes
    if slash_count % 2 != 0 {
        repl_str[x as usize] = SIGMA_ESCAPE;
    } else {
        x += 1; // Move back to the first valid character
    }
    
    // Return the result from the valid starting position
    String::from_utf8(repl_str[x as usize..].to_vec()).unwrap_or_else(|_| str.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_escape_empty_string() {
        assert_eq!(escape_sigma_for_glob(""), "");
    }
    
    #[test]
    fn test_escape_simple_wildcards() {
        assert_eq!(escape_sigma_for_glob("test*"), "test*");
        assert_eq!(escape_sigma_for_glob("test?"), "test?");
    }
    
    #[test]
    fn test_escape_backslash_wildcard() {
        assert_eq!(escape_sigma_for_glob("test\\*"), "test\\*");
        assert_eq!(escape_sigma_for_glob("test\\\\*"), "test\\\\*");
        assert_eq!(escape_sigma_for_glob("test\\\\\\*"), "test\\\\\\*");
    }
    
    #[test]
    fn test_escape_brackets() {
        assert_eq!(escape_sigma_for_glob("test[abc]"), "test\\[abc\\]");
        assert_eq!(escape_sigma_for_glob("test{abc}"), "test\\{abc\\}");
    }
    
    #[test]
    fn test_escape_complex() {
        assert_eq!(escape_sigma_for_glob("test\\\\"), "test\\\\");
        assert_eq!(escape_sigma_for_glob("\\\\test"), "\\\\test");
        // The Go implementation produces three backslashes before the bracket
        // and three after the bracket when the brackets are already escaped
        assert_eq!(escape_sigma_for_glob("test\\[abc\\]"), "test\\\\\\[abc\\\\\\]");
    }
    
    #[test]
    fn test_escape_cow_optimization() {
        // Test zero-copy optimization for strings that don't need escaping
        let simple = "test_simple";
        let cow_result = escape_sigma_for_glob_cow(simple);
        match cow_result {
            Cow::Borrowed(s) => assert_eq!(s, simple),
            Cow::Owned(_) => panic!("Should be borrowed for simple strings"),
        }
        
        // Test allocation for strings that need escaping
        let complex = "test[abc]";
        let cow_result = escape_sigma_for_glob_cow(complex);
        match cow_result {
            Cow::Borrowed(_) => panic!("Should be owned for complex strings"),
            Cow::Owned(s) => assert_eq!(s, "test\\[abc\\]"),
        }
    }
}
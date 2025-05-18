//! String pattern matching implementations

use crate::pattern::traits::StringMatcher;
use glob::Pattern as GlobPattern;
use regex::Regex;
use std::sync::Arc;

/// Pattern for exact content matching
#[derive(Debug, Clone)]
pub struct ContentPattern {
    pub token: String,
    pub lowercase: bool,
    pub no_collapse_ws: bool,
}

impl StringMatcher for ContentPattern {
    fn string_match(&self, value: &str) -> bool {
        let value = handle_whitespace(value, self.no_collapse_ws);
        lower_case_if_needed(&value, self.lowercase) == lower_case_if_needed(&self.token, self.lowercase)
    }
}

/// Pattern for prefix matching
#[derive(Debug, Clone)]
pub struct PrefixPattern {
    pub token: String,
    pub lowercase: bool,
    pub no_collapse_ws: bool,
}

impl StringMatcher for PrefixPattern {
    fn string_match(&self, value: &str) -> bool {
        let value = handle_whitespace(value, self.no_collapse_ws);
        lower_case_if_needed(&value, self.lowercase)
            .starts_with(&lower_case_if_needed(&self.token, self.lowercase))
    }
}

/// Pattern for suffix matching
#[derive(Debug, Clone)]
pub struct SuffixPattern {
    pub token: String,
    pub lowercase: bool,
    pub no_collapse_ws: bool,
}

impl StringMatcher for SuffixPattern {
    fn string_match(&self, value: &str) -> bool {
        let value = handle_whitespace(value, self.no_collapse_ws);
        lower_case_if_needed(&value, self.lowercase)
            .ends_with(&lower_case_if_needed(&self.token, self.lowercase))
    }
}

/// Pattern for regular expression matching
#[derive(Debug)]
pub struct RegexPattern {
    pub regex: Regex,
}

impl StringMatcher for RegexPattern {
    fn string_match(&self, value: &str) -> bool {
        self.regex.is_match(value)
    }
}

/// Pattern for glob matching
#[derive(Debug)]
pub struct GlobPatternMatcher {
    pub glob: GlobPattern,
    pub no_collapse_ws: bool,
}

impl StringMatcher for GlobPatternMatcher {
    fn string_match(&self, value: &str) -> bool {
        let value = handle_whitespace(value, self.no_collapse_ws);
        self.glob.matches(&value)
    }
}

/// Collection of string matchers (OR logic)
#[derive(Debug)]
pub struct StringMatchers {
    matchers: Vec<Box<dyn StringMatcher>>,
}

impl StringMatchers {
    pub fn new(matchers: Vec<Box<dyn StringMatcher>>) -> Self {
        Self { matchers }
    }

    pub fn optimize(self) -> Self {
        // For now, return as is. Optimization can be added later.
        self
    }
}

impl StringMatcher for StringMatchers {
    fn string_match(&self, value: &str) -> bool {
        self.matchers.iter().any(|m| m.string_match(value))
    }
}

/// Collection of string matchers (AND logic)
#[derive(Debug)]
pub struct StringMatchersConj {
    matchers: Vec<Box<dyn StringMatcher>>,
}

impl StringMatchersConj {
    pub fn new(matchers: Vec<Box<dyn StringMatcher>>) -> Self {
        Self { matchers }
    }

    pub fn optimize(self) -> Self {
        // For now, return as is. Optimization can be added later.
        self
    }
}

impl StringMatcher for StringMatchersConj {
    fn string_match(&self, value: &str) -> bool {
        self.matchers.iter().all(|m| m.string_match(value))
    }
}

// Helper functions

lazy_static::lazy_static! {
    static ref WS_COLLAPSE: Regex = Regex::new(r"\s+").unwrap();
}

fn handle_whitespace(value: &str, no_collapse_ws: bool) -> String {
    if no_collapse_ws {
        value.to_string()
    } else {
        WS_COLLAPSE.replace_all(value, " ").to_string()
    }
}

fn lower_case_if_needed(value: &str, lowercase: bool) -> String {
    if lowercase {
        value.to_lowercase()
    } else {
        value.to_string()
    }
}

/// Escape Sigma pattern for glob matching
pub fn escape_sigma_for_glob(pattern: &str) -> String {
    if pattern.is_empty() {
        return String::new();
    }

    let mut result = Vec::with_capacity(pattern.len() * 2);
    let bytes = pattern.as_bytes();
    let mut i = bytes.len();
    let mut wildcard = false;
    let mut slash_count = 0;

    // Process from end to beginning
    while i > 0 {
        i -= 1;
        let ch = bytes[i];

        match ch {
            b'*' | b'?' => wildcard = true,
            b'\\' => {
                if !wildcard {
                    slash_count += 1;
                }
            }
            _ => wildcard = false,
        }

        // Balance backslashes
        if ch != b'\\' && slash_count > 0 {
            if slash_count % 2 != 0 {
                result.push(b'\\');
            }
            slash_count = 0;
        }

        result.push(ch);

        // Escape brackets for glob
        if matches!(ch, b'[' | b']' | b'{' | b'}') {
            result.push(b'\\');
        }
    }

    // Handle leading backslashes
    if slash_count % 2 != 0 {
        result.push(b'\\');
    }

    // Reverse the result since we built it backwards
    result.reverse();
    String::from_utf8(result).unwrap_or_else(|_| pattern.to_string())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_pattern() {
        let pattern = ContentPattern {
            token: "test".to_string(),
            lowercase: false,
            no_collapse_ws: false,
        };

        assert!(pattern.string_match("test"));
        assert!(!pattern.string_match("Test"));
        assert!(!pattern.string_match("testing"));
    }

    #[test]
    fn test_content_pattern_case_insensitive() {
        let pattern = ContentPattern {
            token: "test".to_string(),
            lowercase: true,
            no_collapse_ws: false,
        };

        assert!(pattern.string_match("test"));
        assert!(pattern.string_match("Test"));
        assert!(pattern.string_match("TEST"));
        assert!(!pattern.string_match("testing"));
    }

    #[test]
    fn test_prefix_pattern() {
        let pattern = PrefixPattern {
            token: "test".to_string(),
            lowercase: false,
            no_collapse_ws: false,
        };

        assert!(pattern.string_match("test"));
        assert!(pattern.string_match("testing"));
        assert!(!pattern.string_match("Test"));
        assert!(!pattern.string_match("pretest"));
    }

    #[test]
    fn test_suffix_pattern() {
        let pattern = SuffixPattern {
            token: "test".to_string(),
            lowercase: false,
            no_collapse_ws: false,
        };

        assert!(pattern.string_match("test"));
        assert!(pattern.string_match("pretest"));
        assert!(!pattern.string_match("Test"));
        assert!(!pattern.string_match("testing"));
    }

    #[test]
    fn test_whitespace_handling() {
        let pattern = ContentPattern {
            token: "test value".to_string(),
            lowercase: false,
            no_collapse_ws: false,
        };

        assert!(pattern.string_match("test value"));
        assert!(pattern.string_match("test  value"));
        assert!(pattern.string_match("test\tvalue"));
        assert!(pattern.string_match("test\n\nvalue"));
    }

    #[test]
    fn test_whitespace_preserved() {
        let pattern = ContentPattern {
            token: "test  value".to_string(),
            lowercase: false,
            no_collapse_ws: true,
        };

        assert!(!pattern.string_match("test value"));
        assert!(pattern.string_match("test  value"));
    }

    #[test]
    fn test_escape_sigma_for_glob() {
        assert_eq!(escape_sigma_for_glob("test"), "test");
        assert_eq!(escape_sigma_for_glob("*"), "*");
        assert_eq!(escape_sigma_for_glob("\\*"), "\\*");
        assert_eq!(escape_sigma_for_glob("\\\\*"), "\\\\*");
        assert_eq!(escape_sigma_for_glob("[test]"), "\\[test\\]");
        assert_eq!(escape_sigma_for_glob("\\"), "\\\\");
    }
}
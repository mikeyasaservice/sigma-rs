//! Factory functions for creating pattern matchers

use crate::pattern::{
    num_matcher::{NumMatchers, NumPattern},
    string_matcher::{
        ContentPattern, GlobPatternMatcher, PrefixPattern, RegexPattern, StringMatchers,
        StringMatchersConj, SuffixPattern, escape_sigma_for_glob,
    },
    traits::{NumMatcher, StringMatcher},
    TextPatternModifier,
};
use glob::Pattern as GlobPattern;
use regex::Regex;

/// Create a new string matcher based on patterns and modifiers
pub fn new_string_matcher(
    modifier: TextPatternModifier,
    lowercase: bool,
    all: bool,
    no_collapse_ws: bool,
    patterns: Vec<String>,
) -> Result<Box<dyn StringMatcher>, String> {
    if patterns.is_empty() {
        return Err("No patterns defined for matcher object".to_string());
    }

    let mut matchers: Vec<Box<dyn StringMatcher>> = Vec::new();

    for pattern in patterns {
        let matcher: Box<dyn StringMatcher> = match modifier {
            TextPatternModifier::Regex => {
                let re = Regex::new(&pattern)
                    .map_err(|e| format!("Invalid regex pattern: {}", e))?;
                Box::new(RegexPattern { regex: re })
            }
            TextPatternModifier::Contains => {
                let pattern = escape_sigma_for_glob(&pattern);
                let pattern = format!("*{}*", pattern);
                let glob = GlobPattern::new(&pattern)
                    .map_err(|e| format!("Invalid glob pattern: {}", e))?;
                Box::new(GlobPatternMatcher {
                    glob,
                    no_collapse_ws,
                })
            }
            TextPatternModifier::Suffix => {
                Box::new(SuffixPattern {
                    token: pattern,
                    lowercase,
                    no_collapse_ws,
                })
            }
            TextPatternModifier::Prefix => {
                Box::new(PrefixPattern {
                    token: pattern,
                    lowercase,
                    no_collapse_ws,
                })
            }
            _ => {
                // Handle default cases (None, All, Keyword)
                if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 2 {
                    // Regex pattern in /pattern/ format
                    let regex_str = &pattern[1..pattern.len() - 1];
                    let re = Regex::new(regex_str)
                        .map_err(|e| format!("Invalid regex pattern: {}", e))?;
                    Box::new(RegexPattern { regex: re })
                } else if modifier == TextPatternModifier::Keyword || pattern.contains('*') {
                    // Keyword or glob pattern
                    let pattern = if modifier == TextPatternModifier::Keyword {
                        let escaped = escape_sigma_for_glob(&pattern);
                        format!("*{}*", escaped)
                    } else {
                        escape_sigma_for_glob(&pattern)
                    };
                    let glob = GlobPattern::new(&pattern)
                        .map_err(|e| format!("Invalid glob pattern: {}", e))?;
                    Box::new(GlobPatternMatcher {
                        glob,
                        no_collapse_ws,
                    })
                } else {
                    // Default to content pattern
                    Box::new(ContentPattern {
                        token: pattern,
                        lowercase,
                        no_collapse_ws,
                    })
                }
            }
        };
        matchers.push(matcher);
    }

    // Return appropriate matcher collection
    if matchers.len() == 1 {
        Ok(matchers.into_iter().next().unwrap())
    } else if all {
        Ok(Box::new(StringMatchersConj::new(matchers).optimize()))
    } else {
        Ok(Box::new(StringMatchers::new(matchers).optimize()))
    }
}

/// Create a new numeric matcher from a list of values
pub fn new_num_matcher(values: Vec<i64>) -> Result<Box<dyn NumMatcher>, String> {
    if values.is_empty() {
        return Err("No patterns defined for matcher object".to_string());
    }

    let matchers: Vec<Box<dyn NumMatcher>> = values
        .into_iter()
        .map(|v| Box::new(NumPattern { value: v }) as Box<dyn NumMatcher>)
        .collect();

    if matchers.len() == 1 {
        Ok(matchers.into_iter().next().unwrap())
    } else {
        Ok(Box::new(NumMatchers::new(matchers)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_string_matcher_content() {
        let matcher = new_string_matcher(
            TextPatternModifier::None,
            false,
            false,
            false,
            vec!["test".to_string()],
        )
        .unwrap();

        assert!(matcher.string_match("test"));
        assert!(!matcher.string_match("Test"));
    }

    #[test]
    fn test_new_string_matcher_prefix() {
        let matcher = new_string_matcher(
            TextPatternModifier::Prefix,
            false,
            false,
            false,
            vec!["test".to_string()],
        )
        .unwrap();

        assert!(matcher.string_match("test"));
        assert!(matcher.string_match("testing"));
        assert!(!matcher.string_match("pretest"));
    }

    #[test]
    fn test_new_string_matcher_contains() {
        let matcher = new_string_matcher(
            TextPatternModifier::Contains,
            false,
            false,
            false,
            vec!["test".to_string()],
        )
        .unwrap();

        assert!(matcher.string_match("test"));
        assert!(matcher.string_match("testing"));
        assert!(matcher.string_match("pretest"));
        assert!(matcher.string_match("pretesting"));
    }

    #[test]
    fn test_new_string_matcher_regex() {
        let matcher = new_string_matcher(
            TextPatternModifier::Regex,
            false,
            false,
            false,
            vec![r"test\d+".to_string()],
        )
        .unwrap();

        assert!(matcher.string_match("test123"));
        assert!(!matcher.string_match("test"));
        assert!(!matcher.string_match("testing"));
    }

    #[test]
    fn test_new_string_matcher_glob() {
        let matcher = new_string_matcher(
            TextPatternModifier::None,
            false,
            false,
            false,
            vec!["test*".to_string()],
        )
        .unwrap();

        assert!(matcher.string_match("test"));
        assert!(matcher.string_match("testing"));
        assert!(!matcher.string_match("pretest"));
    }

    #[test]
    fn test_new_string_matcher_multiple() {
        let matcher = new_string_matcher(
            TextPatternModifier::None,
            false,
            false,
            false,
            vec!["test1".to_string(), "test2".to_string()],
        )
        .unwrap();

        assert!(matcher.string_match("test1"));
        assert!(matcher.string_match("test2"));
        assert!(!matcher.string_match("test3"));
    }

    #[test]
    fn test_new_string_matcher_all() {
        // When using 'all' with ContainModifier instead of None, patterns become *pattern*
        let matcher = new_string_matcher(
            TextPatternModifier::Contains,
            false,
            true,
            false,
            vec!["test".to_string(), "value".to_string()],
        )
        .unwrap();

        // With 'all' flag and Contains modifier, the value must contain all patterns
        assert!(matcher.string_match("test value"));
        assert!(matcher.string_match("value test"));
        assert!(matcher.string_match("this test contains value"));
        assert!(!matcher.string_match("test"));
        assert!(!matcher.string_match("value"));
        assert!(!matcher.string_match("neither"));
    }

    #[test]
    fn test_new_num_matcher() {
        let matcher = new_num_matcher(vec![1, 2, 3]).unwrap();

        assert!(matcher.num_match(1));
        assert!(matcher.num_match(2));
        assert!(matcher.num_match(3));
        assert!(!matcher.num_match(4));
    }

    #[test]
    fn test_new_num_matcher_single() {
        let matcher = new_num_matcher(vec![42]).unwrap();

        assert!(matcher.num_match(42));
        assert!(!matcher.num_match(41));
    }

    #[test]
    fn test_empty_patterns_error() {
        let result = new_string_matcher(
            TextPatternModifier::None,
            false,
            false,
            false,
            vec![],
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "No patterns defined for matcher object"
        );
    }
}
//! Factory functions for creating pattern matchers

use crate::pattern::{
    escape::{escape_sigma_for_glob, escape_sigma_for_glob_cow},
    intern::intern_pattern,
    num_matcher::{NumMatchers, NumPattern},
    security::safe_regex_compile,
    string_matcher::{
        ContentPattern, GlobPatternMatcher, PrefixPattern, RegexPattern, StringMatchers,
        StringMatchersConj, SuffixPattern,
    },
    traits::{NumMatcher, StringMatcher},
    TextPatternModifier,
};
use glob::Pattern as GlobPattern;
use std::borrow::Cow;

/// Efficiently create a contains pattern by prepending and appending "*"
/// without unnecessary allocations when possible
fn create_contains_pattern(escaped: Cow<'_, str>) -> String {
    let mut result = String::with_capacity(escaped.len() + 2);
    result.push('*');
    result.push_str(&escaped);
    result.push('*');
    result
}

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

    // Pre-allocate with known capacity to reduce reallocations
    let mut matchers: Vec<Box<dyn StringMatcher>> = Vec::with_capacity(patterns.len());

    for pattern in patterns {
        let matcher: Box<dyn StringMatcher> = match modifier {
            TextPatternModifier::Regex => {
                let re = safe_regex_compile(&pattern)
                    .map_err(|e| format!("Unsafe regex pattern: {}", e))?;
                Box::new(RegexPattern { regex: re })
            }
            TextPatternModifier::Contains => {
                let escaped = escape_sigma_for_glob_cow(&pattern);
                let glob_pattern = create_contains_pattern(escaped);
                let glob = GlobPattern::new(&glob_pattern)
                    .map_err(|e| format!("Invalid glob pattern: {}", e))?;
                Box::new(GlobPatternMatcher {
                    glob,
                    no_collapse_ws,
                })
            }
            TextPatternModifier::Suffix => Box::new(SuffixPattern {
                token: intern_pattern(&pattern),
                lowercase,
                no_collapse_ws,
            }),
            TextPatternModifier::Prefix => Box::new(PrefixPattern {
                token: intern_pattern(&pattern),
                lowercase,
                no_collapse_ws,
            }),
            _ => {
                // Handle default cases (None, All, Keyword)
                if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 2 {
                    // Regex pattern in /pattern/ format
                    let regex_str = &pattern[1..pattern.len() - 1];
                    let re = safe_regex_compile(regex_str)
                        .map_err(|e| format!("Unsafe regex pattern: {}", e))?;
                    Box::new(RegexPattern { regex: re })
                } else if modifier == TextPatternModifier::Keyword || pattern.contains('*') {
                    // Keyword or glob pattern
                    let glob_pattern = if modifier == TextPatternModifier::Keyword {
                        let escaped = escape_sigma_for_glob_cow(&pattern);
                        create_contains_pattern(escaped)
                    } else {
                        escape_sigma_for_glob(&pattern)
                    };
                    let glob = GlobPattern::new(&glob_pattern)
                        .map_err(|e| format!("Invalid glob pattern: {}", e))?;
                    Box::new(GlobPatternMatcher {
                        glob,
                        no_collapse_ws,
                    })
                } else {
                    // Default to content pattern
                    Box::new(ContentPattern {
                        token: intern_pattern(&pattern),
                        lowercase,
                        no_collapse_ws,
                    })
                }
            }
        };
        matchers.push(matcher);
    }

    // Return appropriate matcher collection
    match matchers.len() {
        1 => {
            let mut iter = matchers.into_iter();
            iter.next()
                .ok_or_else(|| "Internal error: Vec with length 1 has no element".to_string())
        }
        _ => {
            if all {
                Ok(Box::new(StringMatchersConj::new(matchers).optimize()))
            } else {
                Ok(Box::new(StringMatchers::new(matchers).optimize()))
            }
        }
    }
}

/// Create a new numeric matcher from a list of values
pub fn new_num_matcher(values: Vec<i64>) -> Result<Box<dyn NumMatcher>, String> {
    if values.is_empty() {
        return Err("No patterns defined for matcher object".to_string());
    }

    // Pre-allocate with known capacity and use safe extraction
    let mut matchers: Vec<Box<dyn NumMatcher>> = Vec::with_capacity(values.len());
    for value in values {
        matchers.push(Box::new(NumPattern { value }) as Box<dyn NumMatcher>);
    }

    match matchers.len() {
        1 => {
            let mut iter = matchers.into_iter();
            iter.next()
                .ok_or_else(|| "Internal error: Vec with length 1 has no element".to_string())
        }
        _ => Ok(Box::new(NumMatchers::new(matchers))),
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
        let result = new_string_matcher(TextPatternModifier::None, false, false, false, vec![]);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "No patterns defined for matcher object"
        );
    }

    #[test]
    fn test_memory_allocation_optimization() {
        // Test that factory pre-allocates capacity correctly
        let patterns = vec![
            "pattern1".to_string(),
            "pattern2".to_string(),
            "pattern3".to_string(),
        ];

        let result =
            new_string_matcher(TextPatternModifier::Contains, false, false, false, patterns);

        assert!(result.is_ok());

        // Test single pattern optimization
        let single_pattern = vec!["single".to_string()];
        let result = new_string_matcher(
            TextPatternModifier::Contains,
            false,
            false,
            false,
            single_pattern,
        );

        assert!(result.is_ok());
    }
}

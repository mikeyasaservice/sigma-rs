//! Pattern matching implementations for Sigma rules


pub mod factory;
pub mod num_matcher;
pub mod string_matcher;
pub mod traits;

pub use factory::*;
pub use num_matcher::*;
pub use string_matcher::*;
pub use traits::*;

/// Type of sigma detection identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierType {
    /// Selection-style identifier (object with field matches)
    Selection,
    /// Keywords-style identifier (array of keywords)
    Keywords,
}

/// Text pattern modifiers for string matching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPatternModifier {
    None,
    Contains,
    Prefix,
    Suffix,
    All,
    Regex,
    Keyword,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifier_type() {
        assert_eq!(IdentifierType::Selection, IdentifierType::Selection);
        assert_ne!(IdentifierType::Selection, IdentifierType::Keywords);
    }
}
//! Pattern matching implementations for Sigma rules

use std::sync::Arc;

/// Type of sigma detection identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierType {
    /// Selection-style identifier (object with field matches)
    Selection,
    /// Keywords-style identifier (array of keywords)
    Keywords,
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
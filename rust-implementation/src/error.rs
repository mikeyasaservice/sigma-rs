/// Error types for the Sigma rule engine
use thiserror::Error;
use std::fmt::Display;

#[derive(Error, Debug)]
pub enum SigmaError {
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Lexer error: {0}")]
    Lexer(String),
    
    #[error("Invalid token sequence: expected {expected}, found {found}")]
    InvalidTokenSequence {
        expected: String,
        found: String,
    },
    
    #[error("Missing detection field")]
    MissingDetection,
    
    #[error("Missing condition in detection")]
    MissingCondition,
    
    #[error("Missing condition item: {key}")]
    MissingConditionItem { key: String },
    
    #[error("Unsupported token: {0}")]
    UnsupportedToken(String),
    
    #[error("Rule not found: {0}")]
    RuleNotFound(String),
    
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),
    
    #[error("Invalid rule format: {0}")]
    InvalidRule(String),
    
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),
    
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    
    #[error("Glob pattern error: {0}")]
    Glob(#[from] glob::PatternError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Kafka error: {0}")]
    Kafka(String),
    
    #[error("Invalid configuration: {0}")]
    Configuration(String),
    
    #[error("Runtime error: {0}")]
    Runtime(String),
}

/// Result type alias for Sigma operations
pub type Result<T> = std::result::Result<T, SigmaError>;

/// Error for bulk parse operations
#[derive(Error, Debug)]
pub struct BulkParseError {
    pub errors: Vec<ParseError>,
}

impl Display for BulkParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse {} rules", self.errors.len())?;
        for (i, err) in self.errors.iter().enumerate() {
            write!(f, "\n  [{}] {}: {}", i + 1, err.path, err.error)?;
        }
        Ok(())
    }
}

/// Individual parse error
#[derive(Debug)]
pub struct ParseError {
    pub path: String,
    pub error: SigmaError,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.error)
    }
}

/// Error chain helper for adding context
pub trait ErrorContext<T> {
    /// Add context to an error
    fn context(self, msg: impl Into<String>) -> Result<T>;
    
    /// Add context with format
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T> ErrorContext<T> for Result<T> {
    fn context(self, msg: impl Into<String>) -> Result<T> {
        self.map_err(|e| SigmaError::Runtime(format!("{}: {}", msg.into(), e)))
    }
    
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| SigmaError::Runtime(format!("{}: {}", f(), e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_display() {
        let err = SigmaError::Parse("invalid syntax".to_string());
        assert_eq!(err.to_string(), "Parse error: invalid syntax");
        
        let err = SigmaError::InvalidTokenSequence {
            expected: "identifier".to_string(),
            found: "keyword".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid token sequence: expected identifier, found keyword");
    }
    
    #[test]
    fn test_bulk_parse_error() {
        let bulk = BulkParseError {
            errors: vec![
                ParseError {
                    path: "rule1.yml".to_string(),
                    error: SigmaError::MissingDetection,
                },
                ParseError {
                    path: "rule2.yml".to_string(),
                    error: SigmaError::MissingCondition,
                },
            ],
        };
        
        let display = bulk.to_string();
        assert!(display.contains("Failed to parse 2 rules"));
        assert!(display.contains("rule1.yml"));
        assert!(display.contains("rule2.yml"));
    }
    
    #[test]
    fn test_error_context() {
        let result: Result<()> = Err(SigmaError::Parse("test".to_string()));
        let with_context = result.context("while parsing rule");
        assert!(with_context.unwrap_err().to_string().contains("while parsing rule"));
    }
}
/// Error types for the Sigma rule engine
use thiserror::Error;
use std::fmt::Display;

/// Main error type for Sigma rule engine operations
#[derive(Error, Debug)]
pub enum SigmaError {
    /// Error during rule parsing
    #[error("Parse error: {0}")]
    Parse(String),
    
    /// Error from the lexical analyzer
    #[error("Lexer error: {0}")]
    Lexer(String),
    
    /// Invalid token sequence encountered during parsing
    #[error("Invalid token sequence: expected {expected}, found {found}")]
    InvalidTokenSequence {
        /// Expected token or token type
        expected: String,
        /// Actual token found
        found: String,
    },
    
    /// Detection field is missing from the rule
    #[error("Missing detection field")]
    MissingDetection,
    
    /// Condition is missing from the detection section
    #[error("Missing condition in detection")]
    MissingCondition,
    
    /// Referenced condition item is missing
    #[error("Missing condition item: {key}")]
    MissingConditionItem { 
        /// Key of the missing condition item
        key: String 
    },
    
    /// Token type is not supported
    #[error("Unsupported token: {0}")]
    UnsupportedToken(String),
    
    /// Rule with specified ID or name not found
    #[error("Rule not found: {0}")]
    RuleNotFound(String),
    
    /// Pattern syntax is invalid
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),
    
    /// Rule format does not conform to Sigma specification
    #[error("Invalid rule format: {0}")]
    InvalidRule(String),
    
    /// YAML parsing failed
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    
    /// JSON parsing failed
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),
    
    /// Regular expression compilation failed
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    
    /// Glob pattern compilation failed
    #[error("Glob pattern error: {0}")]
    Glob(#[from] glob::PatternError),
    
    /// IO operation failed
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Kafka/Redpanda operation failed
    #[error("Kafka error: {0}")]
    Kafka(String),
    
    /// Configuration is invalid or incomplete
    #[error("Invalid configuration: {0}")]
    Configuration(String),
    
    /// Runtime execution error
    #[error("Runtime error: {0}")]
    Runtime(String),
}

/// Result type alias for Sigma operations
pub type Result<T> = std::result::Result<T, SigmaError>;

/// Error for bulk parse operations
#[derive(Error, Debug)]
pub struct BulkParseError {
    /// Collection of parse errors encountered during bulk parsing
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
    /// Path to the file that failed to parse
    pub path: String,
    /// The specific error encountered
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
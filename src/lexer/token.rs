use glob::Pattern;

/// Token types in Sigma expressions
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    // Helpers for internal stuff
    /// Error token
    Error = 0,
    /// Unsupported token type
    Unsupported = 1,
    /// Empty/nil token
    Nil = 2,

    // User-defined word
    /// Regular identifier
    Identifier = 3,
    /// Identifier containing wildcards
    IdentifierWithWildcard = 4,
    /// Special identifier "them"
    IdentifierAll = 5,

    // Literals
    /// End of file literal
    LitEof = 6,

    // Separators
    /// Left parenthesis separator
    SepLpar = 7,
    /// Right parenthesis separator
    SepRpar = 8,
    /// Pipe separator
    SepPipe = 9,

    // Operators
    /// Equals operator
    OpEq = 10,
    /// Greater than operator
    OpGt = 11,
    /// Greater than or equal operator
    OpGte = 12,
    /// Less than operator
    OpLt = 13,
    /// Less than or equal operator
    OpLte = 14,

    // Keywords
    /// AND keyword
    KeywordAnd = 15,
    /// OR keyword
    KeywordOr = 16,
    /// NOT keyword
    KeywordNot = 17,
    /// Aggregation keyword (sum, min, max, etc.)
    KeywordAgg = 18,

    // Statements
    /// "1 of" statement
    StmtOneOf = 19,
    /// "all of" statement
    StmtAllOf = 20,
}

impl Token {
    /// Get the literal representation of the token
    pub fn literal(&self) -> &'static str {
        match self {
            Token::Identifier | Token::IdentifierWithWildcard => "keywords",
            Token::IdentifierAll => "them",
            Token::SepLpar => "(",
            Token::SepRpar => ")",
            Token::SepPipe => "|",
            Token::OpEq => "=",
            Token::OpGt => ">",
            Token::OpGte => ">=",
            Token::OpLt => "<",
            Token::OpLte => "<=",
            Token::KeywordAnd => "and",
            Token::KeywordOr => "or",
            Token::KeywordNot => "not",
            Token::StmtAllOf => "all of",
            Token::StmtOneOf => "1 of",
            Token::LitEof | Token::Nil => "",
            _ => "Err",
        }
    }

    /// Get the rune representation of separator tokens
    pub fn rune(&self) -> Option<char> {
        match self {
            Token::SepLpar => Some('('),
            Token::SepRpar => Some(')'),
            Token::SepPipe => Some('|'),
            _ => None,
        }
    }

    /// Check if this is a valid keyword token
    pub fn from_keyword(s: &str) -> Option<Token> {
        if s.eq_ignore_ascii_case("and") {
            Some(Token::KeywordAnd)
        } else if s.eq_ignore_ascii_case("or") {
            Some(Token::KeywordOr)
        } else if s.eq_ignore_ascii_case("not") {
            Some(Token::KeywordNot)
        } else if s.eq_ignore_ascii_case("sum")
            || s.eq_ignore_ascii_case("min")
            || s.eq_ignore_ascii_case("max")
            || s.eq_ignore_ascii_case("count")
            || s.eq_ignore_ascii_case("avg")
        {
            Some(Token::KeywordAgg)
        } else if s.eq_ignore_ascii_case("them") {
            Some(Token::IdentifierAll)
        } else if s == "1 of" {
            Some(Token::StmtOneOf)
        } else {
            None
        }
    }
}

/// Lexical token with its value
#[derive(Debug, Clone)]
pub struct Item {
    /// The token type
    pub token: Token,
    /// The token value
    pub value: String,
    glob_val: Option<Pattern>,
    glob_compile_failed: bool,
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token && self.value == other.value
    }
}

impl Item {
    /// Create a new item
    pub fn new(token: Token, value: String) -> Self {
        Self {
            token,
            value,
            glob_val: None,
            glob_compile_failed: false,
        }
    }

    /// Get the compiled glob pattern for this item
    pub fn glob(&mut self) -> Option<&Pattern> {
        if self.glob_val.is_none() && !self.glob_compile_failed {
            let pattern_str = escape_sigma_for_glob(&self.value);
            match Pattern::new(&pattern_str) {
                Ok(glob) => self.glob_val = Some(glob),
                Err(_) => {
                    self.glob_compile_failed = true;
                    return None;
                }
            }
        }
        self.glob_val.as_ref()
    }
}

/// Escape Sigma wildcards for glob patterns
fn escape_sigma_for_glob(s: &str) -> String {
    // This is a placeholder - implement the actual escaping logic
    // based on the Go version's escapeSigmaForGlob function
    s.to_string()
}

/// Check if a keyword is valid based on the given string
pub fn check_keyword(input: &str) -> Token {
    if input.is_empty() {
        return Token::Nil;
    }

    if input.eq_ignore_ascii_case("and") {
        Token::KeywordAnd
    } else if input.eq_ignore_ascii_case("or") {
        Token::KeywordOr
    } else if input.eq_ignore_ascii_case("not") {
        Token::KeywordNot
    } else if input.eq_ignore_ascii_case("sum")
        || input.eq_ignore_ascii_case("min")
        || input.eq_ignore_ascii_case("max")
        || input.eq_ignore_ascii_case("count")
        || input.eq_ignore_ascii_case("avg")
    {
        Token::KeywordAgg
    } else if input.eq_ignore_ascii_case("them") {
        Token::IdentifierAll
    } else if input == "1 of" {
        Token::StmtOneOf
    } else {
        // Special case for "all of" - check without allocating
        let trimmed = input.trim();
        if trimmed.len() >= 6 {
            let (first, rest) = trimmed.split_at(3);
            if first.eq_ignore_ascii_case("all") && rest.trim().eq_ignore_ascii_case("of") {
                return Token::StmtAllOf;
            }
        }

        // Check if the identifier contains wildcards
        if input.contains('*') || input.contains('?') {
            Token::IdentifierWithWildcard
        } else {
            Token::Identifier
        }
    }
}

/// Rerun the state machine
/// Takes a channel receiver and emits tokens on the given channel
pub fn emit(
    _to: &Sender<Item>,
    _token: Token,
    _val: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Placeholder for the emit function
    Ok(())
}

use tokio::sync::mpsc::UnboundedSender as Sender;

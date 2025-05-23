use glob::Pattern;

/// Token types in Sigma expressions
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    // Helpers for internal stuff
    Error = 0,
    Unsupported = 1,
    Nil = 2,

    // User-defined word
    Identifier = 3,
    IdentifierWithWildcard = 4,
    IdentifierAll = 5,

    // Literals
    LitEof = 6,

    // Separators
    SepLpar = 7,
    SepRpar = 8,
    SepPipe = 9,

    // Operators
    OpEq = 10,
    OpGt = 11,
    OpGte = 12,
    OpLt = 13,
    OpLte = 14,

    // Keywords
    KeywordAnd = 15,
    KeywordOr = 16,
    KeywordNot = 17,
    KeywordAgg = 18,

    // Statements
    StmtOneOf = 19,
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
            || s.eq_ignore_ascii_case("avg") {
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
    pub token: Token,
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
        || input.eq_ignore_ascii_case("avg") {
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
pub fn emit(_to: &Sender<Item>, _token: Token, _val: String) -> Result<(), Box<dyn std::error::Error>> {
    // Placeholder for the emit function
    Ok(())
}

use tokio::sync::mpsc::UnboundedSender as Sender;
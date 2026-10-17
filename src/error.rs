use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum EzError {
    LexerError(String),
    ParserError(String),
    TypeError(String),
    RuntimeError(String),
    ImportError(String),
    /// An `Err` or `None` propagated with `?` out of the main file.
    UnhandledError(String),
}

impl fmt::Display for EzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EzError::LexerError(message) => write!(f, "syntax error: {}", message),
            EzError::ParserError(message) => write!(f, "syntax error: {}", message),
            EzError::TypeError(message) => write!(f, "type error: {}", message),
            EzError::RuntimeError(message) => write!(f, "panic: {}", message),
            EzError::ImportError(message) => write!(f, "import error: {}", message),
            EzError::UnhandledError(message) => write!(f, "error: {}", message),
        }
    }
}

impl std::error::Error for EzError {}

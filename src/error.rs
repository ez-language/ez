#[derive(Debug)]
pub enum EzError {
    LexerError(String),
    ParserError(String),
    TypeError(String),
    RuntimeError(String),
}

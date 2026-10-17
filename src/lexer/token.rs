#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Identifier(String),
    Int(i64),
    Float(f64),
    StringLiteral(String),
    /// Raw content between backticks, still with `${...}` and escape characters. The parser interprets it.
    TemplateString(String),
    Boolean(bool),
    Var,
    Const,
    Type,
    Interface,
    Import,
    From,
    If,
    Else,
    For,
    In,
    Is,
    While,
    Func,
    Return,
    Async,
    Await,
    Match,
    Switch,
    Case,
    Default,
    Break,
    Continue,
    Fallthrough,
    And,
    Or,
    Not,
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    SlashSlash,
    Percent,
    Ampersand,
    Pipe,
    Caret,
    Tilde,
    ShiftLeft,
    ShiftRight,
    Equal,
    EqualEqual,
    PlusEqual,
    MinusEqual,
    StarEqual,
    StarStarEqual,
    SlashEqual,
    SlashSlashEqual,
    PercentEqual,
    Bang,
    BangEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Arrow,
    FatArrow,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Semicolon,
    Colon,
    /// A character that does not belong to the language.
    Illegal(char),
    EOF,
}

impl Token {
    /// Tokens that end an operand; used to tell `//` (integer division) apart from a comment.
    pub fn ends_operand(&self) -> bool {
        matches!(
            self,
            Token::Identifier(_)
                | Token::Int(_)
                | Token::Float(_)
                | Token::StringLiteral(_)
                | Token::TemplateString(_)
                | Token::Boolean(_)
                | Token::RParen
                | Token::RBracket
                | Token::RBrace
        )
    }

    /// Keyword spelling, so keywords can still be used as member names (`regex.match(...)`).
    pub fn keyword_name(&self) -> Option<&'static str> {
        let name = match self {
            Token::Var => "var",
            Token::Const => "const",
            Token::Type => "type",
            Token::Interface => "interface",
            Token::Import => "import",
            Token::From => "from",
            Token::If => "if",
            Token::Else => "else",
            Token::For => "for",
            Token::In => "in",
            Token::Is => "is",
            Token::While => "while",
            Token::Func => "function",
            Token::Return => "return",
            Token::Async => "async",
            Token::Await => "await",
            Token::Match => "match",
            Token::Default => "default",
            Token::Break => "break",
            Token::Continue => "continue",
            Token::Fallthrough => "fallthrough",
            Token::And => "and",
            Token::Or => "or",
            Token::Not => "not",
            _ => return None,
        };
        Some(name)
    }
}

/// A token together with the position (1-based) where it starts.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub line: usize,
    pub column: usize,
}

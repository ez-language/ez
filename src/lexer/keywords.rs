use crate::lexer::token::Token;

pub fn lookup_ident(ident: &str) -> Token {
    match ident {
        "var" => Token::Var,
        "const" => Token::Const,
        "type" => Token::Type,
        "if" => Token::If,
        "else" => Token::Else,
        "for" => Token::For,
        "in" => Token::In,
        "while" => Token::While,
        "function" => Token::Func,
        "return" => Token::Return,
        "async" => Token::Async,
        "await" => Token::Await,
        "match" => Token::Match,
        "default" => Token::Default,
        "break" => Token::Break,
        "continue" => Token::Continue,
        "fallthrough" => Token::Fallthrough,
        "true" => Token::Boolean(true),
        "false" => Token::Boolean(false),
        "and" => Token::And,
        "or" => Token::Or,
        "not" => Token::Not,
        _ => Token::Identifier(ident.to_string()),
    }
}
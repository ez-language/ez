use crate::lexer::token::Token;

pub fn lookup_ident(ident: &str) -> Token {
    match ident {
        "var" => Token::Var,
        "if" => Token::If,
        "else" => Token::Else,
        "for" => Token::For,
        "in" => Token::In,
        "while" => Token::While,
        "func" => Token::Func,
        "return" => Token::Return,
        "async" => Token::Async,
        "await" => Token::Await,
        "match" => Token::Match,
        "switch" => Token::Switch,
        "case" => Token::Case,
        "default" => Token::Default,
        "break" => Token::Break,
        "fallthrough" => Token::Fallthrough,
        "true" => Token::Boolean(true),
        "false" => Token::Boolean(false),
        "null" => Token::Null,
        "and" => Token::And,
        "or" => Token::Or,
        "not" => Token::Not,
        _ => Token::Identifier(ident.to_string()),
    }
}

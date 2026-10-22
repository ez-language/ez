pub mod token;
pub mod keywords;

use token::{SpannedToken, Token};
use keywords::lookup_ident;

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
        }
    }

    fn current(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    fn advance(&mut self) {
        if let Some(c) = self.current() {
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        self.position += 1;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Consumes the current character and returns a `token`.
    fn single(&mut self, token: Token) -> Token {
        self.advance();
        token
    }

    /// Consumes the current character; if the next character is `second`, consumes that as well and returns `double`.
    fn one_or_two(&mut self, second: char, double: Token, single: Token) -> Token {
        self.advance();
        if self.current() == Some(second) {
            self.advance();
            double
        } else {
            single
        }
    }

    /// Reads all tokens, including the final EOF.
    pub fn tokenize(&mut self) -> Vec<Token> {
        self.tokenize_spanned()
            .into_iter()
            .map(|spanned| spanned.token)
            .collect()
    }

    /// Reads all tokens with their positions, including the final EOF.
    pub fn tokenize_spanned(&mut self) -> Vec<SpannedToken> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_spanned();
            let is_eof = token.token == Token::EOF;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        tokens
    }

    pub fn next_token(&mut self) -> Token {
        self.next_spanned().token
    }

    fn next_spanned(&mut self) -> SpannedToken {
        loop {
            self.skip_whitespace();
            if self.current() == Some('/') && self.peek() == Some('*') {
                self.skip_block_comment();
            } else if self.current() == Some('/') && self.peek() == Some('/') {
                self.skip_line_comment();
            } else {
                break;
            }
        }

        let line = self.line;
        let column = self.column;
        let token = self.read_token();
        SpannedToken {
            token,
            line,
            column,
        }
    }

    fn read_token(&mut self) -> Token {
        match self.current() {
            Some(c) if c.is_alphabetic() || c == '_' => self.read_identifier_or_keyword(),
            Some(c) if c.is_ascii_digit() => self.read_number(),
            Some('"') => self.read_string('"'),
            Some('\'') => self.read_string('\''),
            Some('`') => self.read_template_string(),

            Some('=') => {
                self.advance();
                match self.current() {
                    Some('=') => self.single(Token::EqualEqual),
                    Some('>') => self.single(Token::FatArrow),
                    _ => Token::Equal,
                }
            }
            Some('-') => {
                self.advance();
                match self.current() {
                    Some('>') => self.single(Token::Arrow),
                    Some('=') => self.single(Token::MinusEqual),
                    _ => Token::Minus,
                }
            }
            Some('*') => {
                self.advance();
                match self.current() {
                    Some('*') => self.one_or_two('=', Token::StarStarEqual, Token::StarStar),
                    Some('=') => self.single(Token::StarEqual),
                    _ => Token::Star,
                }
            }
            Some('/') => self.one_or_two('=', Token::SlashEqual, Token::Slash),
            Some('>') => {
                self.advance();
                match self.current() {
                    Some('=') => self.single(Token::GreaterEqual),
                    Some('>') => self.single(Token::ShiftRight),
                    _ => Token::Greater,
                }
            }
            Some('<') => {
                self.advance();
                match self.current() {
                    Some('=') => self.single(Token::LessEqual),
                    Some('<') => self.single(Token::ShiftLeft),
                    _ => Token::Less,
                }
            }
            Some('!') => self.one_or_two('=', Token::BangEqual, Token::Bang),
            Some('+') => self.one_or_two('=', Token::PlusEqual, Token::Plus),
            Some('%') => self.one_or_two('=', Token::PercentEqual, Token::Percent),
            Some('&') => self.single(Token::Ampersand),
            Some('|') => self.single(Token::Pipe),
            Some('^') => self.single(Token::Caret),
            Some('~') => self.single(Token::Tilde),
            Some('(') => self.single(Token::LParen),
            Some(')') => self.single(Token::RParen),
            Some('{') => self.single(Token::LBrace),
            Some('}') => self.single(Token::RBrace),
            Some('[') => self.single(Token::LBracket),
            Some(']') => self.single(Token::RBracket),
            Some(',') => self.single(Token::Comma),
            Some('.') => self.single(Token::Dot),
            Some(';') => self.single(Token::Semicolon),
            Some(':') => self.single(Token::Colon),
            Some('?') => self.single(Token::Question),

            Some(c) => self.single(Token::Illegal(c)),
            None => Token::EOF,
        }
    }

    fn read_identifier_or_keyword(&mut self) -> Token {
        let start = self.position;
        while let Some(c) = self.current() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let ident: String = self.input[start..self.position].iter().collect();
        lookup_ident(&ident)
    }

    fn read_number(&mut self) -> Token {
        let start = self.position;
        let mut has_dot = false;

        while let Some(c) = self.current() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.'
                && !has_dot
                && self.peek().map_or(false, |next| next.is_ascii_digit())
            {
                has_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        // Exponent: `1e308`, `2.5E-3`.
        let mut has_exponent = false;
        if matches!(self.current(), Some('e' | 'E')) {
            let digit_at = if matches!(self.peek(), Some('+' | '-')) { 2 } else { 1 };
            if self
                .input
                .get(self.position + digit_at)
                .is_some_and(|c| c.is_ascii_digit())
            {
                has_exponent = true;
                for _ in 0..digit_at {
                    self.advance();
                }
                while self.current().is_some_and(|c| c.is_ascii_digit()) {
                    self.advance();
                }
            }
        }

        let num_str: String = self.input[start..self.position].iter().collect();
        if !has_dot && !has_exponent {
            if let Ok(value) = num_str.parse::<i64>() {
                return Token::Int(value);
            }
        }
        Token::Float(num_str.parse::<f64>().unwrap_or(0.0))
    }

    /// Reads a string delimited by `quote` (double or single quotes).
    fn read_string(&mut self, quote: char) -> Token {
        self.advance();
        let mut result = String::new();
        while let Some(c) = self.current() {
            if c == quote {
                self.advance();
                break;
            }
            if c == '\\' {
                self.advance();
                if let Some(esc) = self.current() {
                    result.push(match esc {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        '0' => '\0',
                        other => other,
                    });
                    self.advance();
                }
            } else {
                result.push(c);
                self.advance();
            }
        }
        Token::StringLiteral(result)
    }

    /// Reads a template string between backticks. Stores the raw content (escapes and `${...}`
    /// intact); the parser separates text and expressions.
    fn read_template_string(&mut self) -> Token {
        self.advance();
        let mut raw = String::new();
        while let Some(c) = self.current() {
            match c {
                '`' => {
                    self.advance();
                    break;
                }
                '\\' => {
                    raw.push(c);
                    self.advance();
                    if let Some(next) = self.current() {
                        raw.push(next);
                        self.advance();
                    }
                }
                other => {
                    raw.push(other);
                    self.advance();
                }
            }
        }
        Token::TemplateString(raw)
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.current() {
            self.advance();
            if c == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) {
        self.advance();
        self.advance();

        while let Some(c) = self.current() {
            if c == '*' && self.peek() == Some('/') {
                self.advance();
                self.advance();
                break;
            }
            self.advance();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(source: &str) -> Vec<Token> {
        Lexer::new(source).tokenize()
    }

    #[test]
    fn integers_and_floats_are_distinct() {
        assert_eq!(lex("42 2.5"), vec![Token::Int(42), Token::Float(2.5), Token::EOF]);
    }

    #[test]
    fn scientific_notation() {
        assert_eq!(
            lex("1e3 2.5E-2 1e+2"),
            vec![Token::Float(1000.0), Token::Float(0.025), Token::Float(100.0), Token::EOF]
        );
        // `e` not followed by digits is a separate identifier.
        assert_eq!(
            lex("2e"),
            vec![Token::Int(2), Token::Identifier("e".to_string()), Token::EOF]
        );
    }

    #[test]
    fn comment_after_code_is_skipped() {
        assert_eq!(
            lex("print(x) // 1 (comment)"),
            vec![
                Token::Identifier("print".to_string()),
                Token::LParen,
                Token::Identifier("x".to_string()),
                Token::RParen,
                Token::EOF
            ]
        );
    }

    #[test]
    fn double_slash_is_always_a_comment() {
        assert_eq!(
            lex("a //b\nc //= 2"),
            vec![
                Token::Identifier("a".to_string()),
                Token::Identifier("c".to_string()),
                Token::EOF
            ]
        );
        assert_eq!(
            lex("a / b /= 2"),
            vec![
                Token::Identifier("a".to_string()),
                Token::Slash,
                Token::Identifier("b".to_string()),
                Token::SlashEqual,
                Token::Int(2),
                Token::EOF
            ]
        );
    }

    #[test]
    fn comment_at_start_of_line() {
        assert_eq!(lex("a\n//b"), vec![Token::Identifier("a".to_string()), Token::EOF]);
    }

    #[test]
    fn multi_character_operators() {
        assert_eq!(
            lex("** **= << >> & | ^ ~ %="),
            vec![
                Token::StarStar,
                Token::StarStarEqual,
                Token::ShiftLeft,
                Token::ShiftRight,
                Token::Ampersand,
                Token::Pipe,
                Token::Caret,
                Token::Tilde,
                Token::PercentEqual,
                Token::EOF
            ]
        );
    }

    #[test]
    fn tokens_have_positions() {
        let tokens = Lexer::new("a\n  b").tokenize_spanned();
        assert_eq!((tokens[0].line, tokens[0].column), (1, 1));
        assert_eq!((tokens[1].line, tokens[1].column), (2, 3));
    }
}

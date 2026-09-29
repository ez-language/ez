pub mod token;
pub mod keywords;

use token::Token;
use keywords::lookup_ident;

pub struct Lexer {
    input: Vec<char>,
    position: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            position: 0,
        }
    }

    fn current(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    fn advance(&mut self) {
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
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            let is_eof = token == Token::EOF;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        tokens
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        match self.current() {
            Some('/') if self.peek() == Some('/') => {
                self.skip_line_comment();
                self.next_token()
            }
            Some('/') if self.peek() == Some('*') => {
                self.skip_block_comment();
                self.next_token()
            }

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
            Some('!') => self.one_or_two('=', Token::BangEqual, Token::Bang),
            Some('>') => self.one_or_two('=', Token::GreaterEqual, Token::Greater),
            Some('<') => self.one_or_two('=', Token::LessEqual, Token::Less),
            Some('+') => self.one_or_two('=', Token::PlusEqual, Token::Plus),
            Some('*') => self.one_or_two('=', Token::StarEqual, Token::Star),
            Some('/') => self.one_or_two('=', Token::SlashEqual, Token::Slash),
            Some('%') => self.single(Token::Percent),
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

        let num_str: String = self.input[start..self.position].iter().collect();
        let number = num_str.parse::<f64>().unwrap_or(0.0);
        Token::Number(number)
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
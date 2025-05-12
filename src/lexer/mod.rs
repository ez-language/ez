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
            Some('"') => self.read_string(),
            Some(c) => {
                let token = match c {
                    '=' => {
                        if self.peek() == Some('=') {
                            self.advance();
                            self.advance();
                            Token::EqualEqual
                        } else if self.peek() == Some('>') {
                            self.advance();
                            self.advance();
                            Token::FatArrow
                        } else {
                            self.advance();
                            Token::Equal
                        }
                    }
                    '!' => {
                        if self.peek() == Some('=') {
                            self.advance();
                            self.advance();
                            Token::BangEqual
                        } else {
                            self.advance();
                            Token::Bang
                        }
                    }
                    '>' => {
                        if self.peek() == Some('=') {
                            self.advance();
                            self.advance();
                            Token::GreaterEqual
                        } else {
                            self.advance();
                            Token::Greater
                        }
                    }
                    '<' => {
                        if self.peek() == Some('=') {
                            self.advance();
                            self.advance();
                            Token::LessEqual
                        } else {
                            self.advance();
                            Token::Less
                        }
                    }
                    '-' => {
                        if self.peek() == Some('>') {
                            self.advance();
                            self.advance();
                            Token::Arrow
                        } else {
                            self.advance();
                            Token::Minus
                        }
                    }
                    '+' => {
                        self.advance();
                        Token::Plus
                    }
                    '*' => {
                        self.advance();
                        Token::Star
                    }
                    '/' => {
                        self.advance();
                        Token::Slash
                    }
                    '(' => {
                        self.advance();
                        Token::LParen
                    }
                    ')' => {
                        self.advance();
                        Token::RParen
                    }
                    '{' => {
                        self.advance();
                        Token::LBrace
                    }
                    '}' => {
                        self.advance();
                        Token::RBrace
                    }
                    '[' => {
                        self.advance();
                        Token::LBracket
                    }
                    ']' => {
                        self.advance();
                        Token::RBracket
                    }
                    ',' => {
                        self.advance();
                        Token::Comma
                    }
                    '.' => {
                        self.advance();
                        Token::Dot
                    }
                    ';' => {
                        self.advance();
                        Token::Semicolon
                    }
                    ':' => {
                        self.advance();
                        Token::Colon
                    }
                    _ => {
                        self.advance();
                        Token::EOF
                    }
                };
                token
            }
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
            } else if c == '.' && !has_dot {
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

    fn read_string(&mut self) -> Token {
        self.advance();
        let mut result = String::new();
        while let Some(c) = self.current() {
            match c {
                '"' => {
                    self.advance();
                    break;
                }
                '\\' => {
                    self.advance();
                    if let Some(esc) = self.current() {
                        result.push(match esc {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '"' => '"',
                            '\\' => '\\',
                            other => other,
                        });
                        self.advance();
                    }
                }
                other => {
                    result.push(other);
                    self.advance();
                }
            }
        }
        Token::StringLiteral(result)
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

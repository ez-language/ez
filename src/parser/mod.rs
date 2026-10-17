pub mod ast;

use std::cell::RefCell;
use std::rc::Rc;

use crate::error::EzError;
use crate::lexer::token::{SpannedToken, Token};
use crate::lexer::Lexer;
use ast::{
    AssignOperator, AstNode, BinaryOperator, Field, ImportKind, MatchArm, ObjectEntry, Param,
    Pattern, TemplatePart, TypeDef, TypeExpr, UnaryOperator, Variant,
};

static EOF_TOKEN: Token = Token::EOF;

const OR_BP: u8 = 1;
const AND_BP: u8 = 2;
const NOT_BP: u8 = 3;
const EQUALITY_BP: u8 = 4;
const COMPARISON_BP: u8 = 5;
const BIT_OR_BP: u8 = 6;
const BIT_XOR_BP: u8 = 7;
const BIT_AND_BP: u8 = 8;
const SHIFT_BP: u8 = 9;
const ADDITIVE_BP: u8 = 10;
const MULTIPLICATIVE_BP: u8 = 11;
const UNARY_BP: u8 = 12;
const POWER_BP: u8 = 13;

fn error<T>(message: String) -> Result<T, EzError> {
    Err(EzError::ParserError(message))
}

pub struct Parser {
    tokens: Vec<Token>,
    /// Line of each token; 0 when unknown (tokens built without positions).
    lines: Vec<usize>,
    position: usize,
    /// How many loops enclose the current point (used to validate `break` / `continue`).
    loop_depth: usize,
    /// How many functions enclose the current point (used to validate `return`).
    function_depth: usize,
    /// Inside a match arm body, a binary operator on a new line starts the next arm
    /// (`0 => 'zero'` followed by `-1 => 'negative'`).
    newline_ends_expression: bool,
}

impl Parser {
    pub fn new(tokens: &[Token]) -> Self {
        Parser {
            tokens: tokens.to_vec(),
            lines: vec![0; tokens.len()],
            position: 0,
            loop_depth: 0,
            function_depth: 0,
            newline_ends_expression: false,
        }
    }

    pub fn with_positions(tokens: &[SpannedToken]) -> Self {
        Parser {
            tokens: tokens.iter().map(|t| t.token.clone()).collect(),
            lines: tokens.iter().map(|t| t.line).collect(),
            position: 0,
            loop_depth: 0,
            function_depth: 0,
            newline_ends_expression: false,
        }
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&EOF_TOKEN)
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.position + 1).unwrap_or(&EOF_TOKEN)
    }

    fn token_at(&self, index: usize) -> &Token {
        self.tokens.get(index).unwrap_or(&EOF_TOKEN)
    }

    fn line_at(&self, index: usize) -> usize {
        self.lines
            .get(index)
            .or_else(|| self.lines.last())
            .copied()
            .unwrap_or(0)
    }

    fn current_line(&self) -> usize {
        self.line_at(self.position)
    }

    /// Whether the current token starts a new line (only known when positions are available).
    fn at_new_line(&self) -> bool {
        if self.position == 0 {
            return false;
        }
        let current = self.line_at(self.position);
        current != 0 && current != self.line_at(self.position - 1)
    }

    /// Consumes the current token and returns it. At the end of input, returns EOF without advancing.
    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if self.position < self.tokens.len() {
            self.position += 1;
        }
        token
    }

    fn error_here<T>(&self, message: String) -> Result<T, EzError> {
        let line = self.current_line();
        if line == 0 {
            error(message)
        } else {
            error(format!("line {}: {}", line, message))
        }
    }

    fn expect(&mut self, expected: Token, description: &str) -> Result<(), EzError> {
        if *self.current() == expected {
            self.advance();
            Ok(())
        } else {
            self.error_here(format!(
                "expected {}, found {:?}",
                description,
                self.current()
            ))
        }
    }

    fn expect_identifier(&mut self, description: &str) -> Result<String, EzError> {
        match self.current().clone() {
            Token::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            other => self.error_here(format!("expected {}, found {:?}", description, other)),
        }
    }

    /// Like `expect_identifier`, but also accepts keywords (`regex.match`, `{ type: 'x' }`).
    fn expect_name(&mut self, description: &str) -> Result<String, EzError> {
        if let Some(name) = self.current().keyword_name() {
            self.advance();
            return Ok(name.to_string());
        }
        if let Token::Boolean(b) = self.current() {
            let name = b.to_string();
            self.advance();
            return Ok(name);
        }
        self.expect_identifier(description)
    }

    /// Parses something enclosed in brackets, where line breaks never end an expression.
    fn grouped<T>(&mut self, parse: impl FnOnce(&mut Self) -> Result<T, EzError>) -> Result<T, EzError> {
        let saved = std::mem::replace(&mut self.newline_ends_expression, false);
        let result = parse(self);
        self.newline_ends_expression = saved;
        result
    }

    fn skip_semicolons(&mut self) {
        while *self.current() == Token::Semicolon {
            self.advance();
        }
    }

    /// Reads comma-separated items until (without consuming) the `close` token.
    fn parse_comma_separated<T>(
        &mut self,
        close: &Token,
        mut parse_item: impl FnMut(&mut Self) -> Result<T, EzError>,
    ) -> Result<Vec<T>, EzError> {
        let mut items = Vec::new();
        if self.current() != close {
            loop {
                items.push(parse_item(self)?);
                if *self.current() == Token::Comma {
                    self.advance();
                    // Trailing comma.
                    if self.current() == close {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        Ok(items)
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    /// Parses a statement, wrapping it with its line when positions are known.
    fn parse_located_statement(&mut self) -> Result<AstNode, EzError> {
        let line = self.current_line();
        let statement = self.parse_statement()?;
        if line == 0 {
            Ok(statement)
        } else {
            Ok(AstNode::Located {
                line,
                node: Box::new(statement),
            })
        }
    }

    pub fn parse_statement(&mut self) -> Result<AstNode, EzError> {
        match self.current().clone() {
            Token::Break => {
                self.ensure_inside_loop("break")?;
                self.advance();
                Ok(AstNode::Break)
            }
            Token::Continue => {
                self.ensure_inside_loop("continue")?;
                self.advance();
                Ok(AstNode::Continue)
            }
            Token::Return => self.parse_return(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Type => self.parse_type_declaration(),
            Token::Interface => self.error_here(
                "'interface' is a reserved word; to describe the shape of a value, use 'type'"
                    .to_string(),
            ),
            Token::Import => self.parse_import(),
            Token::Export => {
                self.error_here("'export' is only allowed at the top level of a file".to_string())
            }
            Token::Func => self.parse_function_declaration(),
            Token::Var => {
                self.advance();
                let name = self.expect_identifier("variable name")?;
                self.finish_declaration(name, false)
            }
            Token::Const => {
                self.advance();
                let name = self.expect_identifier("constant name")?;
                self.finish_declaration(name, true)
            }
            // `name: type = value`
            Token::Identifier(_) if *self.peek() == Token::Colon => {
                let name = self.expect_identifier("variable name")?;
                self.finish_declaration(name, false)
            }
            _ => self.parse_expression_statement(),
        }
    }

    fn ensure_inside_loop(&self, keyword: &str) -> Result<(), EzError> {
        if self.loop_depth == 0 {
            self.error_here(format!("'{}' can only be used inside a loop", keyword))
        } else {
            Ok(())
        }
    }

    fn parse_return(&mut self) -> Result<AstNode, EzError> {
        if self.function_depth == 0 {
            return self.error_here("'return' can only be used inside a function".to_string());
        }
        self.advance(); // return
        let ends_here = matches!(
            self.current(),
            Token::RBrace | Token::EOF | Token::Semicolon
        ) || self.at_new_line();
        if ends_here {
            return Ok(AstNode::Return(None));
        }
        let value = self.parse_expression(0)?;
        Ok(AstNode::Return(Some(Box::new(value))))
    }

    fn parse_expression_statement(&mut self) -> Result<AstNode, EzError> {
        let expression = self.parse_expression(0)?;

        let op = match self.current() {
            Token::Equal => AssignOperator::Assign,
            Token::PlusEqual => AssignOperator::Add,
            Token::MinusEqual => AssignOperator::Subtract,
            Token::StarEqual => AssignOperator::Multiply,
            Token::SlashEqual => AssignOperator::Divide,
            Token::StarStarEqual => AssignOperator::Power,
            Token::PercentEqual => AssignOperator::Modulo,
            _ => return Ok(expression),
        };

        match expression {
            AstNode::Identifier(_) | AstNode::Index { .. } | AstNode::Member { .. } => {}
            other => {
                return self.error_here(format!("invalid assignment target: {:?}", other));
            }
        }

        self.advance();
        let value = self.parse_expression(0)?;
        Ok(AstNode::Assign {
            target: Box::new(expression),
            op,
            value: Box::new(value),
        })
    }

    /// After the name: `[: type] = value`.
    fn finish_declaration(&mut self, name: String, is_const: bool) -> Result<AstNode, EzError> {
        let ty = if *self.current() == Token::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(Token::Equal, "'=' in declaration")?;
        let value = self.parse_expression(0)?;
        Ok(AstNode::VarDecl {
            name,
            ty,
            value: Box::new(value),
            is_const,
        })
    }

    fn parse_type(&mut self) -> Result<TypeExpr, EzError> {
        let mut ty = if *self.current() == Token::LParen {
            // Function type: `(int, string) => boolean`
            self.advance();
            let params = self.parse_comma_separated(&Token::RParen, |p| p.parse_type())?;
            self.expect(Token::RParen, "')' in function type")?;
            self.expect(Token::FatArrow, "'=>' in function type")?;
            let ret = self.parse_type()?;
            TypeExpr::Function {
                params,
                ret: Box::new(ret),
            }
        } else {
            let mut name = self.expect_identifier("type name")?;
            while *self.current() == Token::Dot {
                self.advance();
                name.push('.');
                name.push_str(&self.expect_identifier("type name after '.'")?);
            }
            if *self.current() == Token::LBracket && *self.peek() != Token::RBracket {
                self.advance();
                let args = self.parse_comma_separated(&Token::RBracket, |p| p.parse_type())?;
                self.expect(Token::RBracket, "']' after the type arguments")?;
                TypeExpr::Generic(name, args)
            } else {
                TypeExpr::Named(name)
            }
        };
        while *self.current() == Token::LBracket && *self.peek() == Token::RBracket {
            self.advance();
            self.advance();
            ty = TypeExpr::Array(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_block(&mut self) -> Result<AstNode, EzError> {
        self.grouped(Self::parse_block_inner)
    }

    fn parse_block_inner(&mut self) -> Result<AstNode, EzError> {
        self.expect(Token::LBrace, "'{'")?;
        let mut statements = Vec::new();
        loop {
            self.skip_semicolons();
            if *self.current() == Token::RBrace {
                break;
            }
            if *self.current() == Token::EOF {
                return self.error_here("unclosed block: expected '}'".to_string());
            }
            statements.push(self.parse_located_statement()?);
        }
        self.advance();
        Ok(AstNode::Block(statements))
    }

    fn parse_loop_body(&mut self) -> Result<AstNode, EzError> {
        self.loop_depth += 1;
        let body = self.parse_block();
        self.loop_depth -= 1;
        body
    }

    /// Parses a function body: `break`/`continue` do not cross function boundaries.
    fn parse_function_body(&mut self, block_only: bool) -> Result<AstNode, EzError> {
        let saved_loops = std::mem::replace(&mut self.loop_depth, 0);
        self.function_depth += 1;
        let body = if block_only || *self.current() == Token::LBrace {
            self.parse_block()
        } else {
            self.parse_expression(0)
        };
        self.function_depth -= 1;
        self.loop_depth = saved_loops;
        body
    }

    /// Conditions must be parenthesized: `(x > 1)`, `not (x > 1)` or `(a) and (b)`.
    fn parse_condition(&mut self, keyword: &str) -> Result<AstNode, EzError> {
        let mut index = self.position;
        while matches!(self.token_at(index), Token::Not | Token::Bang) {
            index += 1;
        }
        if *self.token_at(index) != Token::LParen {
            return self.error_here(format!(
                "expected '(' after '{}': conditions must be in parentheses",
                keyword
            ));
        }
        self.parse_expression(0)
    }

    fn parse_while(&mut self) -> Result<AstNode, EzError> {
        self.advance(); // while
        let condition = self.parse_condition("while")?;
        let body = self.parse_loop_body()?;
        Ok(AstNode::While {
            condition: Box::new(condition),
            body: Box::new(body),
        })
    }

    fn parse_for(&mut self) -> Result<AstNode, EzError> {
        self.advance(); // for
        let variable = self.expect_identifier("variable name for 'for'")?;
        self.expect(Token::In, "'in'")?;
        let iterable = self.parse_expression(0)?;
        let body = self.parse_loop_body()?;
        Ok(AstNode::For {
            variable,
            iterable: Box::new(iterable),
            body: Box::new(body),
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, EzError> {
        self.expect(Token::LParen, "'(' before the parameters")?;
        let params = self.parse_comma_separated(&Token::RParen, |p| {
            let name = p.expect_identifier("parameter name")?;
            let ty = if *p.current() == Token::Colon {
                p.advance();
                Some(p.parse_type()?)
            } else {
                None
            };
            Ok(Param { name, ty })
        })?;
        self.expect(Token::RParen, "')' after the parameters")?;
        Ok(params)
    }

    fn parse_optional_return_type(&mut self) -> Result<Option<TypeExpr>, EzError> {
        if *self.current() == Token::Colon {
            self.advance();
            Ok(Some(self.parse_type()?))
        } else {
            Ok(None)
        }
    }

    fn parse_function_declaration(&mut self) -> Result<AstNode, EzError> {
        self.advance(); // function
        let name = self.expect_identifier("function name")?;
        let params = self.parse_params()?;
        let return_type = self.parse_optional_return_type()?;
        let body = self.parse_function_body(true)?;
        Ok(AstNode::FunctionDecl {
            name,
            params,
            return_type,
            body: Rc::new(body),
        })
    }

    /// Whether the `(` at `open` starts an anonymous function: `(...) =>` or `(...): type =>`.
    fn is_lambda_start(&self, open: usize) -> bool {
        let mut depth = 0;
        let mut index = open;
        loop {
            match self.token_at(index) {
                Token::LParen => depth += 1,
                Token::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Token::EOF => return false,
                _ => {}
            }
            index += 1;
        }
        matches!(self.token_at(index + 1), Token::FatArrow | Token::Colon)
    }

    fn parse_lambda(&mut self) -> Result<AstNode, EzError> {
        let params = self.parse_params()?;
        let return_type = self.parse_optional_return_type()?;
        self.expect(Token::FatArrow, "'=>' in anonymous function")?;
        let body = self.parse_function_body(false)?;
        Ok(AstNode::Lambda {
            params,
            return_type,
            body: Rc::new(body),
        })
    }

    /// `export function`, `export const`, `export var` or `export type`.
    fn parse_export(&mut self) -> Result<AstNode, EzError> {
        let line = self.current_line();
        self.advance(); // export
        if !matches!(
            self.current(),
            Token::Func | Token::Const | Token::Var | Token::Type
        ) {
            return self.error_here(format!(
                "expected 'function', 'const', 'var' or 'type' after 'export', found {:?}",
                self.current()
            ));
        }
        let declaration = AstNode::Export(Box::new(self.parse_statement()?));
        if line == 0 {
            Ok(declaration)
        } else {
            Ok(AstNode::Located {
                line,
                node: Box::new(declaration),
            })
        }
    }

    fn parse_import(&mut self) -> Result<AstNode, EzError> {
        self.advance(); // import
        if *self.current() == Token::LBrace {
            self.advance();
            let names =
                self.parse_comma_separated(&Token::RBrace, |p| p.expect_identifier("imported name"))?;
            self.expect(Token::RBrace, "'}' after the imported names")?;
            self.expect(Token::From, "'from' after the imported names")?;
            let path = self.expect_string("module path")?;
            return Ok(AstNode::Import {
                kind: ImportKind::Names(names),
                path,
            });
        }

        let name = self.expect_identifier("module name")?;
        let path = if *self.current() == Token::From {
            self.advance();
            self.expect_string("module path")?
        } else {
            name.clone()
        };
        Ok(AstNode::Import {
            kind: ImportKind::Module(name),
            path,
        })
    }

    fn expect_string(&mut self, description: &str) -> Result<String, EzError> {
        match self.current().clone() {
            Token::StringLiteral(s) => {
                self.advance();
                Ok(s)
            }
            other => self.error_here(format!("expected {}, found {:?}", description, other)),
        }
    }

    /// Fields `name: type` separated by newlines or commas, up to and including `}`.
    fn parse_fields(&mut self) -> Result<Vec<Field>, EzError> {
        let mut fields: Vec<Field> = Vec::new();
        loop {
            while matches!(self.current(), Token::Comma | Token::Semicolon) {
                self.advance();
            }
            if *self.current() == Token::RBrace {
                self.advance();
                break;
            }
            if *self.current() == Token::EOF {
                return self.error_here("unclosed field list: expected '}'".to_string());
            }
            let name = self.expect_name("field name")?;
            self.expect(Token::Colon, "':' after the field name")?;
            let ty = self.parse_type()?;
            if fields.iter().any(|f| f.name == name) {
                return self.error_here(format!("duplicate field '{}'", name));
            }
            fields.push(Field { name, ty });
        }
        Ok(fields)
    }

    fn parse_type_declaration(&mut self) -> Result<AstNode, EzError> {
        self.advance(); // type
        let name = self.expect_identifier("type name")?;
        self.expect(Token::Equal, "'=' in type declaration")?;

        let mut parts = vec![self.parse_type_part()?];
        while *self.current() == Token::Ampersand {
            self.advance();
            parts.push(self.parse_type_part()?);
        }
        if *self.current() == Token::Pipe {
            return self.error_here(format!(
                "untagged union in type '{}' is not allowed; use a sum type with named variants",
                name
            ));
        }

        let definition = if parts.len() == 1 {
            parts.remove(0)
        } else {
            TypeDef::Intersection(parts)
        };
        Ok(AstNode::TypeDecl { name, definition })
    }

    /// One operand of a type definition: a `{ ... }` body or a type name.
    fn parse_type_part(&mut self) -> Result<TypeDef, EzError> {
        if *self.current() != Token::LBrace {
            return Ok(TypeDef::Alias(self.parse_type()?));
        }
        self.advance(); // {

        match (self.current().clone(), self.peek().clone()) {
            (Token::LBracket, _) => {
                self.advance();
                self.expect_identifier("key name")?;
                self.expect(Token::Colon, "':' after the key name")?;
                let key = self.parse_type()?;
                self.expect(Token::RBracket, "']' after the key type")?;
                self.expect(Token::Colon, "':' before the value type")?;
                let value = self.parse_type()?;
                while matches!(self.current(), Token::Comma | Token::Semicolon) {
                    self.advance();
                }
                self.expect(Token::RBrace, "'}' after the index signature")?;
                Ok(TypeDef::Index { key, value })
            }
            (Token::RBrace, _) | (_, Token::Colon) => Ok(TypeDef::Shape(self.parse_fields()?)),
            _ => {
                let mut variants: Vec<Variant> = Vec::new();
                loop {
                    while matches!(self.current(), Token::Comma | Token::Semicolon) {
                        self.advance();
                    }
                    if *self.current() == Token::RBrace {
                        self.advance();
                        break;
                    }
                    if *self.current() == Token::EOF {
                        return self.error_here(
                            "unclosed type declaration: expected '}'".to_string(),
                        );
                    }
                    let variant = self.parse_variant()?;
                    if variants.iter().any(|v| v.name == variant.name) {
                        return self.error_here(format!("duplicate variant '{}'", variant.name));
                    }
                    variants.push(variant);
                }
                Ok(TypeDef::Sum(variants))
            }
        }
    }

    fn parse_variant(&mut self) -> Result<Variant, EzError> {
        let name = self.expect_identifier("variant name")?;
        let mut fields = Vec::new();
        if *self.current() == Token::LParen {
            self.advance();
            fields = self.parse_comma_separated(&Token::RParen, |p| {
                let field_name = p.expect_name("field name")?;
                p.expect(Token::Colon, "':' after the field name")?;
                let ty = p.parse_type()?;
                Ok(Field {
                    name: field_name,
                    ty,
                })
            })?;
            self.expect(Token::RParen, "')' at the end of the fields")?;
        }
        Ok(Variant { name, fields })
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    /// Binding power, operator and whether it is right-associative.
    fn infix_operator(token: &Token) -> Option<(u8, BinaryOperator, bool)> {
        let entry = match token {
            Token::Or => (OR_BP, BinaryOperator::Or),
            Token::And => (AND_BP, BinaryOperator::And),
            Token::EqualEqual => (EQUALITY_BP, BinaryOperator::Equal),
            Token::BangEqual => (EQUALITY_BP, BinaryOperator::NotEqual),
            Token::Is => (EQUALITY_BP, BinaryOperator::Is),
            Token::In => (COMPARISON_BP, BinaryOperator::In),
            Token::Greater => (COMPARISON_BP, BinaryOperator::Greater),
            Token::GreaterEqual => (COMPARISON_BP, BinaryOperator::GreaterEqual),
            Token::Less => (COMPARISON_BP, BinaryOperator::Less),
            Token::LessEqual => (COMPARISON_BP, BinaryOperator::LessEqual),
            Token::Pipe => (BIT_OR_BP, BinaryOperator::BitOr),
            Token::Caret => (BIT_XOR_BP, BinaryOperator::BitXor),
            Token::Ampersand => (BIT_AND_BP, BinaryOperator::BitAnd),
            Token::ShiftLeft => (SHIFT_BP, BinaryOperator::ShiftLeft),
            Token::ShiftRight => (SHIFT_BP, BinaryOperator::ShiftRight),
            Token::Plus => (ADDITIVE_BP, BinaryOperator::Add),
            Token::Minus => (ADDITIVE_BP, BinaryOperator::Subtract),
            Token::Star => (MULTIPLICATIVE_BP, BinaryOperator::Multiply),
            Token::Slash => (MULTIPLICATIVE_BP, BinaryOperator::Divide),
            Token::Percent => (MULTIPLICATIVE_BP, BinaryOperator::Modulo),
            Token::StarStar => return Some((POWER_BP, BinaryOperator::Power, true)),
            _ => return None,
        };
        Some((entry.0, entry.1, false))
    }

    pub fn parse_expression(&mut self, min_bp: u8) -> Result<AstNode, EzError> {
        let mut left = self.parse_prefix()?;

        loop {
            // Postfix operators bind more tightly than any binary operator. A `(` or `[` on a
            // new line starts a new statement instead of continuing the previous expression.
            if *self.current() == Token::LParen && !self.at_new_line() {
                left = self.parse_call(left)?;
                continue;
            }
            if *self.current() == Token::LBracket && !self.at_new_line() {
                left = self.parse_index(left)?;
                continue;
            }
            if *self.current() == Token::Dot {
                left = self.parse_member(left)?;
                continue;
            }
            if *self.current() == Token::Question {
                self.advance();
                left = AstNode::Try {
                    value: Box::new(left),
                    wrap: RefCell::new(None),
                };
                continue;
            }

            if self.newline_ends_expression && self.at_new_line() {
                break;
            }
            let (bp, op, right_assoc) = match Self::infix_operator(self.current()) {
                Some(entry) => entry,
                None => break,
            };
            if bp < min_bp {
                break;
            }

            self.advance();
            // bp + 1 => left associativity
            let right = self.parse_expression(if right_assoc { bp } else { bp + 1 })?;
            left = AstNode::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<AstNode, EzError> {
        if *self.current() == Token::LParen && self.is_lambda_start(self.position) {
            return self.parse_lambda();
        }
        let line = self.current_line();
        match self.advance() {
            Token::Int(n) => Ok(AstNode::IntLiteral(n)),
            Token::Float(n) if !n.is_finite() => {
                let message = "float literal is too large (float has no infinity)".to_string();
                if line == 0 {
                    error(message)
                } else {
                    error(format!("line {}: {}", line, message))
                }
            }
            Token::Float(n) => Ok(AstNode::FloatLiteral(n)),
            Token::StringLiteral(s) => Ok(AstNode::StringLiteral(s)),
            Token::TemplateString(raw) => parse_template(&raw, line),
            Token::Boolean(b) => Ok(AstNode::BooleanLiteral(b)),
            Token::Identifier(name) => Ok(AstNode::Identifier(name)),
            Token::LBracket => {
                let elements = self.grouped(|p| {
                    p.parse_comma_separated(&Token::RBracket, |p| p.parse_expression(0))
                })?;
                self.expect(Token::RBracket, "']' at the end of the array")?;
                Ok(AstNode::ArrayLiteral(elements))
            }
            Token::LBrace => self.grouped(Self::parse_object_literal),
            Token::If => self.parse_if(),
            Token::Match => self.parse_match(),
            Token::Minus => {
                let operand = self.parse_expression(UNARY_BP)?;
                Ok(AstNode::Unary {
                    op: UnaryOperator::Negate,
                    operand: Box::new(operand),
                })
            }
            Token::Tilde => {
                let operand = self.parse_expression(UNARY_BP)?;
                Ok(AstNode::Unary {
                    op: UnaryOperator::BitNot,
                    operand: Box::new(operand),
                })
            }
            Token::Bang | Token::Not => {
                let operand = self.parse_expression(NOT_BP)?;
                Ok(AstNode::Unary {
                    op: UnaryOperator::Not,
                    operand: Box::new(operand),
                })
            }
            Token::LParen => {
                let inner = self.grouped(|p| p.parse_expression(0))?;
                self.expect(Token::RParen, "')'")?;
                Ok(inner)
            }
            other => {
                let message = format!("unexpected expression: {:?}", other);
                if line == 0 {
                    error(message)
                } else {
                    error(format!("line {}: {}", line, message))
                }
            }
        }
    }

    /// Called after consuming `{`. Entries are separated by commas or newlines.
    fn parse_object_literal(&mut self) -> Result<AstNode, EzError> {
        let mut entries: Vec<ObjectEntry> = Vec::new();
        loop {
            while matches!(self.current(), Token::Comma | Token::Semicolon) {
                self.advance();
            }
            if *self.current() == Token::RBrace {
                self.advance();
                break;
            }
            if *self.current() == Token::EOF {
                return self.error_here("unclosed object: expected '}'".to_string());
            }
            let key = match self.current().clone() {
                Token::StringLiteral(s) => {
                    self.advance();
                    s
                }
                _ => self.expect_name("object key")?,
            };
            self.expect(Token::Colon, "':' after the object key")?;
            let value = self.parse_expression(0)?;
            if entries.iter().any(|e| e.key == key) {
                return self.error_here(format!("duplicate key '{}'", key));
            }
            entries.push(ObjectEntry { key, value });
        }
        Ok(AstNode::ObjectLiteral(entries))
    }

    fn parse_call(&mut self, callee: AstNode) -> Result<AstNode, EzError> {
        self.advance(); // '('
        let args = self.grouped(|p| {
            p.parse_comma_separated(&Token::RParen, |p| p.parse_expression(0))
        })?;
        self.expect(Token::RParen, "')' at the end of the arguments")?;
        Ok(AstNode::Call {
            callee: Box::new(callee),
            args,
        })
    }

    fn parse_index(&mut self, target: AstNode) -> Result<AstNode, EzError> {
        self.advance(); // '['
        let index = self.grouped(|p| p.parse_expression(0))?;
        self.expect(Token::RBracket, "']' after the index")?;
        Ok(AstNode::Index {
            target: Box::new(target),
            index: Box::new(index),
        })
    }

    fn parse_member(&mut self, object: AstNode) -> Result<AstNode, EzError> {
        self.advance(); // '.'
        let name = self.expect_name("member name after '.'")?;
        Ok(AstNode::Member {
            object: Box::new(object),
            name,
        })
    }

    /// Called after consuming `if`.
    fn parse_if(&mut self) -> Result<AstNode, EzError> {
        let condition = self.parse_condition("if")?;
        let then_branch = self.parse_block()?;

        let else_branch = if *self.current() == Token::Else {
            self.advance();
            if *self.current() == Token::If {
                return self.error_here(
                    "ez does not have 'else if'; use 'match' with guards".to_string(),
                );
            }
            Some(Box::new(self.parse_block()?))
        } else {
            None
        };

        Ok(AstNode::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
        })
    }

    /// Called after consuming `match`.
    fn parse_match(&mut self) -> Result<AstNode, EzError> {
        self.expect(Token::LParen, "'(' after 'match'")?;
        let scrutinee = self.parse_expression(0)?;
        self.expect(Token::RParen, "')' after the match value")?;
        self.expect(Token::LBrace, "'{' to open the match arms")?;

        let mut arms = Vec::new();
        loop {
            while matches!(self.current(), Token::Comma | Token::Semicolon) {
                self.advance();
            }
            if *self.current() == Token::RBrace {
                break;
            }
            if *self.current() == Token::EOF {
                return self.error_here("unclosed match: expected '}'".to_string());
            }
            arms.push(self.parse_match_arm()?);
        }
        self.advance();

        if arms.is_empty() {
            return self.error_here("match with no arms".to_string());
        }
        Ok(AstNode::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        })
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, EzError> {
        let pattern = self.parse_pattern()?;

        let guard = if *self.current() == Token::If {
            self.advance();
            self.expect(Token::LParen, "'(' after 'if' in the guard")?;
            let condition = self.parse_expression(0)?;
            self.expect(Token::RParen, "')' after the guard")?;
            Some(condition)
        } else {
            None
        };

        self.expect(Token::FatArrow, "'=>'")?;

        let body = if *self.current() == Token::LBrace {
            self.parse_block()?
        } else {
            self.parse_located_arm_expression()?
        };

        Ok(MatchArm {
            pattern,
            guard,
            body,
        })
    }

    /// Arm bodies may be statements such as `x += 1` or `break`, as well as expressions.
    fn parse_located_arm_expression(&mut self) -> Result<AstNode, EzError> {
        let saved = std::mem::replace(&mut self.newline_ends_expression, true);
        let result = match self.current() {
            Token::Break | Token::Continue | Token::Return => self.parse_statement(),
            _ => self.parse_expression_statement(),
        };
        self.newline_ends_expression = saved;
        result
    }

    fn parse_pattern(&mut self) -> Result<Pattern, EzError> {
        match self.current().clone() {
            Token::Identifier(name) => {
                self.advance();
                if name == "_" {
                    return Ok(Pattern::Wildcard);
                }
                if *self.current() == Token::LParen {
                    self.advance();
                    let args = self.parse_comma_separated(&Token::RParen, |p| p.parse_pattern())?;
                    self.expect(Token::RParen, "')' at the end of the pattern")?;
                    Ok(Pattern::Variant { name, args })
                } else {
                    Ok(Pattern::Identifier(name))
                }
            }
            Token::Int(n) => {
                self.advance();
                Ok(Pattern::Literal(AstNode::IntLiteral(n)))
            }
            Token::Float(n) => {
                self.advance();
                Ok(Pattern::Literal(AstNode::FloatLiteral(n)))
            }
            Token::Minus => {
                self.advance();
                match self.advance() {
                    Token::Int(n) => Ok(Pattern::Literal(AstNode::IntLiteral(-n))),
                    Token::Float(n) => Ok(Pattern::Literal(AstNode::FloatLiteral(-n))),
                    other => self.error_here(format!("unexpected pattern: -{:?}", other)),
                }
            }
            Token::StringLiteral(s) => {
                self.advance();
                Ok(Pattern::Literal(AstNode::StringLiteral(s)))
            }
            Token::Boolean(b) => {
                self.advance();
                Ok(Pattern::Literal(AstNode::BooleanLiteral(b)))
            }
            other => self.error_here(format!("unexpected pattern: {:?}", other)),
        }
    }
}

/// Splits the raw contents of a template string into text and `${...}` expressions.
fn parse_template(raw: &str, line: usize) -> Result<AstNode, EzError> {
    let at_line = |message: String| -> Result<AstNode, EzError> {
        if line == 0 {
            error(message)
        } else {
            error(format!("line {}: {}", line, message))
        }
    };
    let chars: Vec<char> = raw.chars().collect();
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '\\' && i + 1 < chars.len() {
            text.push(match chars[i + 1] {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '0' => '\0',
                other => other,
            });
            i += 2;
        } else if c == '$' && chars.get(i + 1) == Some(&'{') {
            if !text.is_empty() {
                parts.push(TemplatePart::Text(std::mem::take(&mut text)));
            }
            i += 2;

            let start = i;
            let mut depth = 1;
            while i < chars.len() {
                match chars[i] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            if depth != 0 {
                return at_line("unclosed interpolation: expected '}'".to_string());
            }

            let inner: String = chars[start..i].iter().collect();
            let tokens = Lexer::new(&inner).tokenize();
            let mut sub_parser = Parser::new(&tokens);
            let expression = match sub_parser.parse_expression(0) {
                Ok(expression) => expression,
                Err(EzError::ParserError(message)) => {
                    return at_line(format!("in interpolation: {}", message))
                }
                Err(other) => return Err(other),
            };
            if *sub_parser.current() != Token::EOF {
                return at_line(format!(
                    "unexpected token inside interpolation: {:?}",
                    sub_parser.current()
                ));
            }
            parts.push(TemplatePart::Expression(expression));
            i += 1; // '}'
        } else {
            text.push(c);
            i += 1;
        }
    }

    if !text.is_empty() {
        parts.push(TemplatePart::Text(text));
    }
    Ok(AstNode::TemplateString(parts))
}

/// Parses a single expression and requires the input to end there.
pub fn parse(tokens: &[Token]) -> Result<AstNode, EzError> {
    let mut parser = Parser::new(tokens);
    let expression = parser.parse_expression(0)?;
    if *parser.current() != Token::EOF {
        return error(format!(
            "unexpected token after expression: {:?}",
            parser.current()
        ));
    }
    Ok(expression)
}

fn parse_statements(mut parser: Parser) -> Result<Vec<AstNode>, EzError> {
    let mut statements = Vec::new();
    loop {
        parser.skip_semicolons();
        if *parser.current() == Token::EOF {
            break;
        }
        if *parser.current() == Token::Export {
            statements.push(parser.parse_export()?);
        } else {
            statements.push(parser.parse_located_statement()?);
        }
    }
    Ok(statements)
}

/// Parses an entire file: a sequence of statements.
pub fn parse_program(tokens: &[Token]) -> Result<Vec<AstNode>, EzError> {
    parse_statements(Parser::new(tokens))
}

/// Parses an entire file keeping line information: statements are wrapped in `Located`, and a
/// `(` or `[` at the start of a line is not taken as a call/index of the previous line.
pub fn parse_program_with_positions(tokens: &[SpannedToken]) -> Result<Vec<AstNode>, EzError> {
    parse_statements(Parser::with_positions(tokens))
}

/// Lexes and parses source code.
pub fn parse_source(source: &str) -> Result<Vec<AstNode>, EzError> {
    let tokens = Lexer::new(source).tokenize_spanned();
    if let Some(illegal) = tokens.iter().find(|t| matches!(t.token, Token::Illegal(_))) {
        if let Token::Illegal(c) = illegal.token {
            return Err(EzError::LexerError(format!(
                "line {}: unexpected character '{}'",
                illegal.line, c
            )));
        }
    }
    parse_program_with_positions(&tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_str(source: &str) -> Result<AstNode, EzError> {
        let tokens = Lexer::new(source).tokenize();
        parse(&tokens)
    }

    fn parse_program_str(source: &str) -> Result<Vec<AstNode>, EzError> {
        let tokens = Lexer::new(source).tokenize();
        parse_program(&tokens)
    }

    fn num(n: i64) -> AstNode {
        AstNode::IntLiteral(n)
    }

    fn ident(name: &str) -> AstNode {
        AstNode::Identifier(name.to_string())
    }

    fn bin(left: AstNode, op: BinaryOperator, right: AstNode) -> AstNode {
        AstNode::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    // ---- expressions ----

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        let ast = parse_str("1 + 2 * 3").unwrap();
        let expected = bin(
            num(1),
            BinaryOperator::Add,
            bin(num(2), BinaryOperator::Multiply, num(3)),
        );
        assert_eq!(ast, expected);
    }

    #[test]
    fn subtraction_is_left_associative() {
        let ast = parse_str("10 - 3 - 2").unwrap();
        let expected = bin(
            bin(num(10), BinaryOperator::Subtract, num(3)),
            BinaryOperator::Subtract,
            num(2),
        );
        assert_eq!(ast, expected);
    }

    #[test]
    fn modulo_binds_tighter_than_equality() {
        let ast = parse_str("i % 2 == 0").unwrap();
        let expected = bin(
            bin(ident("i"), BinaryOperator::Modulo, num(2)),
            BinaryOperator::Equal,
            num(0),
        );
        assert_eq!(ast, expected);
    }

    #[test]
    fn unary_binds_tighter_than_binary() {
        let ast = parse_str("-a * b").unwrap();
        let expected = bin(
            AstNode::Unary {
                op: UnaryOperator::Negate,
                operand: Box::new(ident("a")),
            },
            BinaryOperator::Multiply,
            ident("b"),
        );
        assert_eq!(ast, expected);
    }

    #[test]
    fn function_call_with_arguments() {
        let ast = parse_str("f(1, x + 2)").unwrap();
        let expected = AstNode::Call {
            callee: Box::new(ident("f")),
            args: vec![num(1), bin(ident("x"), BinaryOperator::Add, num(2))],
        };
        assert_eq!(ast, expected);
    }

    #[test]
    fn index_and_method_call() {
        let ast = parse_str("names[i]").unwrap();
        assert_eq!(
            ast,
            AstNode::Index {
                target: Box::new(ident("names")),
                index: Box::new(ident("i")),
            }
        );

        let ast = parse_str("names.length()").unwrap();
        assert_eq!(
            ast,
            AstNode::Call {
                callee: Box::new(AstNode::Member {
                    object: Box::new(ident("names")),
                    name: "length".to_string(),
                }),
                args: vec![],
            }
        );
    }

    #[test]
    fn single_quoted_string() {
        let ast = parse_str("'Adult'").unwrap();
        assert_eq!(ast, AstNode::StringLiteral("Adult".to_string()));
    }

    #[test]
    fn template_string_with_interpolation() {
        let ast = parse_str("`Value: ${value}!`").unwrap();
        assert_eq!(
            ast,
            AstNode::TemplateString(vec![
                TemplatePart::Text("Value: ".to_string()),
                TemplatePart::Expression(ident("value")),
                TemplatePart::Text("!".to_string()),
            ])
        );
    }

    #[test]
    fn missing_operand_is_an_error() {
        assert!(parse_str("1 +").is_err());
    }

    #[test]
    fn unclosed_parenthesis_is_an_error() {
        assert!(parse_str("(1 + 2").is_err());
    }

    #[test]
    fn illegal_character_is_an_error() {
        assert!(parse_str("1 + @").is_err());
    }

    // ---- control flow ----

    #[test]
    fn for_loop_with_continue() {
        let source = r#"
            for i in range(0, 10) {
                if (i % 2 == 0) {
                    continue
                }

                print(i)
            }
        "#;
        let program = parse_program_str(source).unwrap();
        assert_eq!(program.len(), 1);
        match &program[0] {
            AstNode::For { variable, body, .. } => {
                assert_eq!(variable.as_str(), "i");
                match &**body {
                    AstNode::Block(statements) => assert_eq!(statements.len(), 2),
                    other => panic!("expected Block, found {:?}", other),
                }
            }
            other => panic!("expected For, found {:?}", other),
        }
    }

    #[test]
    fn while_with_compound_assignment() {
        let source = r#"
            while (count < 5) {
                print(count)

                count += 1
            }
        "#;
        let program = parse_program_str(source).unwrap();
        match &program[0] {
            AstNode::While { body, .. } => match &**body {
                AstNode::Block(statements) => {
                    assert_eq!(statements.len(), 2);
                    assert!(matches!(
                        &statements[1],
                        AstNode::Assign {
                            op: AssignOperator::Add,
                            ..
                        }
                    ));
                }
                other => panic!("expected Block, found {:?}", other),
            },
            other => panic!("expected While, found {:?}", other),
        }
    }

    #[test]
    fn break_outside_loop_is_an_error() {
        assert!(parse_program_str("break").is_err());
        assert!(parse_program_str("continue").is_err());
    }

    #[test]
    fn break_inside_loop_is_ok() {
        assert!(parse_program_str("while (true) { break }").is_ok());
    }

    #[test]
    fn else_if_is_rejected() {
        let result = parse_program_str("if (a) { b } else if (c) { d }");
        assert!(result.is_err());
    }

    #[test]
    fn if_else_as_expression_in_const() {
        let source = "const label: string = if (age > 18) { 'Adult' } else { 'Minor' }";
        let program = parse_program_str(source).unwrap();
        match &program[0] {
            AstNode::VarDecl {
                name,
                ty,
                value,
                is_const,
            } => {
                assert_eq!(name.as_str(), "label");
                assert!(*is_const);
                assert_eq!(*ty, Some(TypeExpr::Named("string".to_string())));
                assert!(matches!(&**value, AstNode::If { .. }));
            }
            other => panic!("expected VarDecl, found {:?}", other),
        }
    }

    #[test]
    fn typed_declaration_without_keyword() {
        let program = parse_program_str("names: string[] = ['Anna', 'Claire']").unwrap();
        match &program[0] {
            AstNode::VarDecl {
                ty,
                value,
                is_const,
                ..
            } => {
                assert!(!*is_const);
                assert_eq!(
                    *ty,
                    Some(TypeExpr::Array(Box::new(TypeExpr::Named(
                        "string".to_string()
                    ))))
                );
                match &**value {
                    AstNode::ArrayLiteral(items) => assert_eq!(items.len(), 2),
                    other => panic!("expected ArrayLiteral, found {:?}", other),
                }
            }
            other => panic!("expected VarDecl, found {:?}", other),
        }
    }

    // ---- sum types & match ----

    #[test]
    fn sum_type_declaration() {
        let source = r#"
            type Shape = {
                Circle(radius: float)
                Rectangle(width: float, height: float)
            }
        "#;
        let program = parse_program_str(source).unwrap();
        match &program[0] {
            AstNode::TypeDecl {
                name,
                definition: TypeDef::Sum(variants),
            } => {
                assert_eq!(name.as_str(), "Shape");
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name.as_str(), "Circle");
                assert_eq!(variants[1].fields.len(), 2);
                assert_eq!(variants[1].fields[0].ty, TypeExpr::Named("float".to_string()));
            }
            other => panic!("expected TypeDecl, found {:?}", other),
        }
    }

    #[test]
    fn match_with_variant_patterns() {
        let source = r#"
            match (shape) {
                Circle(radius) => 3.14 * radius * radius
                Rectangle(width, height) => width * height
            }
        "#;
        let program = parse_program_str(source).unwrap();
        match &program[0] {
            AstNode::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert_eq!(
                    arms[1].pattern,
                    Pattern::Variant {
                        name: "Rectangle".to_string(),
                        args: vec![
                            Pattern::Identifier("width".to_string()),
                            Pattern::Identifier("height".to_string()),
                        ],
                    }
                );
                assert_eq!(
                    arms[1].body,
                    bin(ident("width"), BinaryOperator::Multiply, ident("height"))
                );
            }
            other => panic!("expected Match, found {:?}", other),
        }
    }

    #[test]
    fn match_with_guards_and_wildcard() {
        let source = r#"
            match (age) {
                x if (x > 18) => 'Adult'
                x if (x > 12) => 'Teen'
                _             => 'Minor'
            }
        "#;
        let program = parse_program_str(source).unwrap();
        match &program[0] {
            AstNode::Match { arms, .. } => {
                assert_eq!(arms.len(), 3);
                assert_eq!(arms[0].pattern, Pattern::Identifier("x".to_string()));
                assert!(arms[0].guard.is_some());
                assert!(arms[1].guard.is_some());
                assert_eq!(arms[2].pattern, Pattern::Wildcard);
                assert!(arms[2].guard.is_none());
            }
            other => panic!("expected Match, found {:?}", other),
        }
    }

    #[test]
    fn match_with_unit_variant_and_wildcard() {
        let source = r#"
            match (user) {
                Admin(name) => print(`Admin: ${name}`)
                Guest => print('Guest user')
            }
        "#;
        let program = parse_program_str(source).unwrap();
        match &program[0] {
            AstNode::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert_eq!(arms[1].pattern, Pattern::Identifier("Guest".to_string()));
            }
            other => panic!("expected Match, found {:?}", other),
        }
    }

    #[test]
    fn match_can_be_assigned_to_a_const() {
        let source = r#"
            const label: string = match (result) {
                Ok(value) => `Value: ${value}`
                Err(error) => `Error: ${error}`
            }
        "#;
        let program = parse_program_str(source).unwrap();
        match &program[0] {
            AstNode::VarDecl { value, .. } => {
                assert!(matches!(&**value, AstNode::Match { .. }));
            }
            other => panic!("expected VarDecl, found {:?}", other),
        }
    }

    // ---- declarations ----

    #[test]
    fn function_declaration_and_return() {
        let program =
            parse_program_str("function sum(a: int, b: int): int { return a + b }").unwrap();
        match &program[0] {
            AstNode::FunctionDecl {
                name,
                params,
                return_type,
                body,
            } => {
                assert_eq!(name.as_str(), "sum");
                assert_eq!(params.len(), 2);
                assert_eq!(*return_type, Some(TypeExpr::Named("int".to_string())));
                assert!(matches!(&**body, AstNode::Block(statements) if matches!(statements[0], AstNode::Return(Some(_)))));
            }
            other => panic!("expected FunctionDecl, found {:?}", other),
        }
        assert!(parse_program_str("return 1").is_err());
    }

    #[test]
    fn anonymous_functions() {
        let program = parse_program_str("const double = (x: int) => x * 2").unwrap();
        match &program[0] {
            AstNode::VarDecl { value, .. } => match &**value {
                AstNode::Lambda {
                    params,
                    return_type,
                    body,
                } => {
                    assert_eq!(params[0].name.as_str(), "x");
                    assert!(return_type.is_none());
                    assert_eq!(**body, bin(ident("x"), BinaryOperator::Multiply, num(2)));
                }
                other => panic!("expected Lambda, found {:?}", other),
            },
            other => panic!("expected VarDecl, found {:?}", other),
        }
        assert!(parse_str("(x: int): int => { return x }").is_ok());
        // A parenthesized expression is not a lambda.
        assert_eq!(parse_str("(x)").unwrap(), ident("x"));
    }

    #[test]
    fn object_literal_with_commas_or_newlines() {
        let program = parse_program_str("user = {\n id: 1\n name: 'Alice'\n}").unwrap();
        match &program[0] {
            AstNode::Assign { value, .. } => match &**value {
                AstNode::ObjectLiteral(entries) => {
                    assert_eq!(entries.len(), 2);
                    assert_eq!(entries[1].key.as_str(), "name");
                }
                other => panic!("expected ObjectLiteral, found {:?}", other),
            },
            other => panic!("expected Assign, found {:?}", other),
        }
        assert!(parse_str("{ 'hello': 'olá', 'world': 'mundo' }").is_ok());
    }

    #[test]
    fn type_definitions() {
        let program = parse_program_str(
            "type Person = { name: string  age: int }\n\
             type Dictionary = { [key: string]: string }\n\
             type Dog = Animal & { breed: string }",
        )
        .unwrap();
        assert!(matches!(&program[0], AstNode::TypeDecl { definition: TypeDef::Shape(fields), .. } if fields.len() == 2));
        assert!(matches!(&program[1], AstNode::TypeDecl { definition: TypeDef::Index { .. }, .. }));
        assert!(matches!(&program[2], AstNode::TypeDecl { definition: TypeDef::Intersection(parts), .. } if parts.len() == 2));
        assert!(parse_program_str("interface User { id: int }").is_err());
        assert!(parse_program_str("type Result = string | int").is_err());
    }

    #[test]
    fn generic_and_function_types() {
        let program = parse_program_str("f: (int) => Option[int][] = g").unwrap();
        match &program[0] {
            AstNode::VarDecl { ty: Some(TypeExpr::Function { params, ret }), .. } => {
                assert_eq!(params.len(), 1);
                assert!(matches!(&**ret, TypeExpr::Array(inner) if matches!(&**inner, TypeExpr::Generic(name, _) if name == "Option")));
            }
            other => panic!("expected a function type, found {:?}", other),
        }
    }

    #[test]
    fn imports() {
        let program = parse_program_str(
            "import math\nimport m from 'math.ez'\nimport { add, subtract } from 'math.ez'",
        )
        .unwrap();
        assert_eq!(
            program[0],
            AstNode::Import {
                kind: ImportKind::Module("math".to_string()),
                path: "math".to_string()
            }
        );
        assert!(matches!(&program[2], AstNode::Import { kind: ImportKind::Names(names), .. } if names.len() == 2));
    }

    #[test]
    fn operator_precedence() {
        // `**` is right-associative and binds tighter than unary minus.
        assert_eq!(
            parse_str("-2 ** 3 ** 2").unwrap(),
            AstNode::Unary {
                op: UnaryOperator::Negate,
                operand: Box::new(bin(
                    num(2),
                    BinaryOperator::Power,
                    bin(num(3), BinaryOperator::Power, num(2))
                )),
            }
        );
        // `not` applies to the whole comparison.
        assert!(matches!(
            parse_str("not a > b").unwrap(),
            AstNode::Unary { op: UnaryOperator::Not, .. }
        ));
        assert_eq!(
            parse_str("a | b & c").unwrap(),
            bin(ident("a"), BinaryOperator::BitOr, bin(ident("b"), BinaryOperator::BitAnd, ident("c")))
        );
    }

    #[test]
    fn keywords_as_member_names() {
        assert!(parse_str("regex.match('a', 'b')").is_ok());
    }

    #[test]
    fn lines_are_recorded() {
        let tokens = Lexer::new("x = 1\n\nprint(x)").tokenize_spanned();
        let program = parse_program_with_positions(&tokens).unwrap();
        assert!(matches!(program[1], AstNode::Located { line: 3, .. }));
    }
}
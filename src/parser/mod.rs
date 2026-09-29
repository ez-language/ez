pub mod ast;

use crate::error::EzError;
use crate::lexer::token::Token;
use crate::lexer::Lexer;
use ast::{
    AssignOperator, AstNode, BinaryOperator, Field, MatchArm, Pattern, TemplatePart, TypeExpr,
    UnaryOperator, Variant,
};

static EOF_TOKEN: Token = Token::EOF;

const OR_BP: u8 = 1;
const AND_BP: u8 = 2;
const EQUALITY_BP: u8 = 3;
const COMPARISON_BP: u8 = 4;
const ADDITIVE_BP: u8 = 5;
const MULTIPLICATIVE_BP: u8 = 6;
const UNARY_BP: u8 = 7;

fn error<T>(message: String) -> Result<T, EzError> {
    Err(EzError::ParserError(message))
}

pub struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
    /// How many loops enclose the current point (used to validate `break` / `continue`).
    loop_depth: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Parser {
            tokens,
            position: 0,
            loop_depth: 0,
        }
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&EOF_TOKEN)
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.position + 1).unwrap_or(&EOF_TOKEN)
    }

    /// Consumes the current token and returns it. At the end of input, returns EOF without advancing.
    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if self.position < self.tokens.len() {
            self.position += 1;
        }
        token
    }

    fn expect(&mut self, expected: Token, description: &str) -> Result<(), EzError> {
        if *self.current() == expected {
            self.advance();
            Ok(())
        } else {
            error(format!(
                "expected {}, found {:?}",
                description,
                self.current()
            ))
        }
    }

    fn expect_identifier(&mut self, description: &str) -> Result<String, EzError> {
        match self.advance() {
            Token::Identifier(name) => Ok(name),
            other => error(format!("expected {}, found {:?}", description, other)),
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

    pub fn parse_statement(&mut self) -> Result<AstNode, EzError> {
        match self.current().clone() {
            Token::Break => {
                self.advance();
                self.ensure_inside_loop("break")?;
                Ok(AstNode::Break)
            }
            Token::Continue => {
                self.advance();
                self.ensure_inside_loop("continue")?;
                Ok(AstNode::Continue)
            }
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Type => self.parse_type_declaration(),
            Token::Var => {
                self.advance();
                let name = self.expect_identifier("variable name")?;
                self.finish_declaration(name, false)
            },
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
            error(format!("'{}' can only be used inside a loop", keyword))
        } else {
            Ok(())
        }
    }

    fn parse_expression_statement(&mut self) -> Result<AstNode, EzError> {
        let expression = self.parse_expression(0)?;

        let op = match self.current() {
            Token::Equal => AssignOperator::Assign,
            Token::PlusEqual => AssignOperator::Add,
            Token::MinusEqual => AssignOperator::Subtract,
            Token::StarEqual => AssignOperator::Multiply,
            Token::SlashEqual => AssignOperator::Divide,
            _ => return Ok(expression),
        };

        match expression {
            AstNode::Identifier(_) | AstNode::Index { .. } | AstNode::Member { .. } => {}
            other => {
                return error(format!("invalid assignment target: {:?}", other));
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
        let name = self.expect_identifier("type name")?;
        let mut ty = TypeExpr::Named(name);
        while *self.current() == Token::LBracket && *self.peek() == Token::RBracket {
            self.advance();
            self.advance();
            ty = TypeExpr::Array(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_block(&mut self) -> Result<AstNode, EzError> {
        self.expect(Token::LBrace, "'{'")?;
        let mut statements = Vec::new();
        while *self.current() != Token::RBrace {
            if *self.current() == Token::EOF {
                return error("unclosed block: expected '}'".to_string());
            }
            statements.push(self.parse_statement()?);
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

    fn parse_while(&mut self) -> Result<AstNode, EzError> {
        self.advance(); // while
        self.expect(Token::LParen, "'(' after 'while'")?;
        let condition = self.parse_expression(0)?;
        self.expect(Token::RParen, "')' after the condition")?;
        let body = self.parse_loop_body()?;
        Ok(AstNode::While {
            condition: Box::new(condition),
            body: Box::new(body),
        })
    }

    fn parse_for(&mut self) -> Result<AstNode, EzError> {
        self.advance(); // for
        let variable = self.expect_identifier("variable name for 'for'")?;
        self.expect(Token::In, " 'in' ")?;
        let iterable = self.parse_expression(0)?;
        let body = self.parse_loop_body()?;
        Ok(AstNode::For {
            variable,
            iterable: Box::new(iterable),
            body: Box::new(body),
        })
    }

    fn parse_type_declaration(&mut self) -> Result<AstNode, EzError> {
        self.advance(); // type
        let name = self.expect_identifier("type name")?;
        self.expect(Token::Equal, "'=' in type declaration")?;
        self.expect(Token::LBrace, "'{'")?;

        let mut variants = Vec::new();
        while *self.current() != Token::RBrace {
            if *self.current() == Token::EOF {
                return error("unclosed type declaration: expected '}'".to_string());
            }
            variants.push(self.parse_variant()?);
        }
        self.advance();

        if variants.is_empty() {
            return error(format!("type '{}' has no variants", name));
        }
        Ok(AstNode::TypeDecl { name, variants })
    }

    fn parse_variant(&mut self) -> Result<Variant, EzError> {
        let name = self.expect_identifier("variant name")?;
        let mut fields = Vec::new();
        if *self.current() == Token::LParen {
            self.advance();
            fields = self.parse_comma_separated(&Token::RParen, |p| {
                let field_name = p.expect_identifier("field name")?;
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

    fn infix_operator(token: &Token) -> Option<(u8, BinaryOperator)> {
        let entry = match token {
            Token::Or => (OR_BP, BinaryOperator::Or),
            Token::And => (AND_BP, BinaryOperator::And),
            Token::EqualEqual => (EQUALITY_BP, BinaryOperator::Equal),
            Token::BangEqual => (EQUALITY_BP, BinaryOperator::NotEqual),
            Token::Greater => (COMPARISON_BP, BinaryOperator::Greater),
            Token::GreaterEqual => (COMPARISON_BP, BinaryOperator::GreaterEqual),
            Token::Less => (COMPARISON_BP, BinaryOperator::Less),
            Token::LessEqual => (COMPARISON_BP, BinaryOperator::LessEqual),
            Token::Plus => (ADDITIVE_BP, BinaryOperator::Add),
            Token::Minus => (ADDITIVE_BP, BinaryOperator::Subtract),
            Token::Star => (MULTIPLICATIVE_BP, BinaryOperator::Multiply),
            Token::Slash => (MULTIPLICATIVE_BP, BinaryOperator::Divide),
            Token::Percent => (MULTIPLICATIVE_BP, BinaryOperator::Modulo),
            _ => return None,
        };
        Some(entry)
    }

    pub fn parse_expression(&mut self, min_bp: u8) -> Result<AstNode, EzError> {
        let mut left = self.parse_prefix()?;

        loop {
            // Postfix operators bind more tightly than any binary operator.
            if *self.current() == Token::LParen {
                left = self.parse_call(left)?;
                continue;
            }
            if *self.current() == Token::LBracket {
                left = self.parse_index(left)?;
                continue;
            }
            if *self.current() == Token::Dot {
                left = self.parse_member(left)?;
                continue;
            }

            let (bp, op) = match Self::infix_operator(self.current()) {
                Some(entry) => entry,
                None => break,
            };
            if bp < min_bp {
                break;
            }

            self.advance();
            // bp + 1 => left associativity
            let right = self.parse_expression(bp + 1)?;
            left = AstNode::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<AstNode, EzError> {
        match self.advance() {
            Token::Number(n) => Ok(AstNode::NumberLiteral(n)),
            Token::StringLiteral(s) => Ok(AstNode::StringLiteral(s)),
            Token::TemplateString(raw) => parse_template(&raw),
            Token::Boolean(b) => Ok(AstNode::BooleanLiteral(b)),
            Token::Null => Ok(AstNode::NullLiteral),
            Token::Identifier(name) => Ok(AstNode::Identifier(name)),
            Token::LBracket => {
                let elements = self.parse_comma_separated(&Token::RBracket, |p| {
                    p.parse_expression(0)
                })?;
                self.expect(Token::RBracket, "']' at the end of the array")?;
                Ok(AstNode::ArrayLiteral(elements))
            }
            Token::If => self.parse_if(),
            Token::Match => self.parse_match(),
            Token::Minus => {
                let operand = self.parse_expression(UNARY_BP)?;
                Ok(AstNode::Unary {
                    op: UnaryOperator::Negate,
                    operand: Box::new(operand),
                })
            }
            Token::Bang | Token::Not => {
                let operand = self.parse_expression(UNARY_BP)?;
                Ok(AstNode::Unary {
                    op: UnaryOperator::Not,
                    operand: Box::new(operand),
                })
            }
            Token::LParen => {
                let inner = self.parse_expression(0)?;
                self.expect(Token::RParen, "')'")?;
                Ok(inner)
            }
            other => error(format!("unexpected expression: {:?}", other)),
        }
    }

    fn parse_call(&mut self, callee: AstNode) -> Result<AstNode, EzError> {
        self.advance(); // '('
        let args = self.parse_comma_separated(&Token::RParen, |p| p.parse_expression(0))?;
        self.expect(Token::RParen, "')' at the end of the arguments")?;
        Ok(AstNode::Call {
            callee: Box::new(callee),
            args,
        })
    }

    fn parse_index(&mut self, target: AstNode) -> Result<AstNode, EzError> {
        self.advance(); // '['
        let index = self.parse_expression(0)?;
        self.expect(Token::RBracket, "']' after the index")?;
        Ok(AstNode::Index {
            target: Box::new(target),
            index: Box::new(index),
        })
    }

    fn parse_member(&mut self, object: AstNode) -> Result<AstNode, EzError> {
        self.advance(); // '.'
        let name = self.expect_identifier("member name after '.'")?;
        Ok(AstNode::Member {
            object: Box::new(object),
            name,
        })
    }

    /// Called after consuming `if`.
    fn parse_if(&mut self) -> Result<AstNode, EzError> {
        self.expect(Token::LParen, "'(' after 'if'")?;
        let condition = self.parse_expression(0)?;
        self.expect(Token::RParen, "')' after the condition")?;
        let then_branch = self.parse_block()?;

        let else_branch = if *self.current() == Token::Else {
            self.advance();
            if *self.current() == Token::If {
                return error(
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
        while *self.current() != Token::RBrace {
            if *self.current() == Token::EOF {
                return error("unclosed match: expected '}'".to_string());
            }
            arms.push(self.parse_match_arm()?);
        }
        self.advance();

        if arms.is_empty() {
            return error("match with no arms".to_string());
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
            self.parse_expression(0)?
        };

        Ok(MatchArm {
            pattern,
            guard,
            body,
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, EzError> {
        match self.advance() {
            Token::Identifier(name) => {
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
            other => error(format!("unexpected pattern: {:?}", other)),
        }
    }
}

/// Splits the raw contents of a template string into text and `${...}` expressions.
fn parse_template(raw: &str) -> Result<AstNode, EzError> {
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
                return error("unclosed interpolation: expected '}'".to_string());
            }

            let inner: String = chars[start..i].iter().collect();
            let tokens = Lexer::new(&inner).tokenize();
            let mut sub_parser = Parser::new(&tokens);
            let expression = sub_parser.parse_expression(0)?;
            if *sub_parser.current() != Token::EOF {
                return error(format!(
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

/// Parses an entire file: a sequence of statements.
pub fn parse_program(tokens: &[Token]) -> Result<Vec<AstNode>, EzError> {
    let mut parser = Parser::new(tokens);
    let mut statements = Vec::new();
    while *parser.current() != Token::EOF {
        statements.push(parser.parse_statement()?);
    }
    Ok(statements)
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

    fn num(n: f64) -> AstNode {
        AstNode::NumberLiteral(n)
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
            num(1.0),
            BinaryOperator::Add,
            bin(num(2.0), BinaryOperator::Multiply, num(3.0)),
        );
        assert_eq!(ast, expected);
    }

    #[test]
    fn subtraction_is_left_associative() {
        let ast = parse_str("10 - 3 - 2").unwrap();
        let expected = bin(
            bin(num(10.0), BinaryOperator::Subtract, num(3.0)),
            BinaryOperator::Subtract,
            num(2.0),
        );
        assert_eq!(ast, expected);
    }

    #[test]
    fn modulo_binds_tighter_than_equality() {
        let ast = parse_str("i % 2 == 0").unwrap();
        let expected = bin(
            bin(ident("i"), BinaryOperator::Modulo, num(2.0)),
            BinaryOperator::Equal,
            num(0.0),
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
            args: vec![num(1.0), bin(ident("x"), BinaryOperator::Add, num(2.0))],
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
            AstNode::TypeDecl { name, variants } => {
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
}
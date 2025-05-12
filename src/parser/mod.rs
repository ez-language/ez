pub mod ast;

use ast::AstNode;

pub fn parse(tokens: &[crate::lexer::token::Token]) -> Option<AstNode> {
    println!("Parsing tokens: {:?}", tokens);
    None
}

#[derive(Debug)]
pub enum AstNode {
    NumberLiteral(f64),
    Identifier(String),
    BinaryOp {
        left: Box<AstNode>,
        op: String,
        right: Box<AstNode>,
    },
}

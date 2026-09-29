#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Negate,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOperator {
    Assign,
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// Type written in the code: `int`, `string[]`, `Result`...
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(String),
    Array(Box<TypeExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
}

/// Variant of a sum type: `Ok(value: string)` or `Guest`.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_`
    Wildcard,
    /// Loose name: can be a binding (`x`) or a variant without fields (`Guest`).
    /// Semantic analysis decides, as it is the one that knows the declared variants.
    Identifier(String),
    /// `Ok(value)`, `Rectangle(width, height)`, with nested patterns.
    Variant { name: String, args: Vec<Pattern> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<AstNode>,
    pub body: AstNode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    Text(String),
    Expression(AstNode),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    // Expressions
    NumberLiteral(f64),
    StringLiteral(String),
    TemplateString(Vec<TemplatePart>),
    BooleanLiteral(bool),
    NullLiteral,
    ArrayLiteral(Vec<AstNode>),
    Identifier(String),
    Unary {
        op: UnaryOperator,
        operand: Box<AstNode>,
    },
    BinaryOp {
        left: Box<AstNode>,
        op: BinaryOperator,
        right: Box<AstNode>,
    },
    Call {
        callee: Box<AstNode>,
        args: Vec<AstNode>,
    },
    Index {
        target: Box<AstNode>,
        index: Box<AstNode>,
    },
    Member {
        object: Box<AstNode>,
        name: String,
    },
    /// `if` is both an expression and a statement. `else_branch` is always a `Block`.
    If {
        condition: Box<AstNode>,
        then_branch: Box<AstNode>,
        else_branch: Option<Box<AstNode>>,
    },
    Match {
        scrutinee: Box<AstNode>,
        arms: Vec<MatchArm>,
    },
    Block(Vec<AstNode>),

    // Statements
    While {
        condition: Box<AstNode>,
        body: Box<AstNode>,
    },
    For {
        variable: String,
        iterable: Box<AstNode>,
        body: Box<AstNode>,
    },
    Break,
    Continue,
    Assign {
        target: Box<AstNode>,
        op: AssignOperator,
        value: Box<AstNode>,
    },
    VarDecl {
        name: String,
        ty: Option<TypeExpr>,
        value: Box<AstNode>,
        is_const: bool,
    },
    TypeDecl {
        name: String,
        variants: Vec<Variant>,
    },
}
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Negate,
    Not,
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Modulo,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    And,
    Or,
    Is,
    In,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
}

impl BinaryOperator {
    pub fn symbol(&self) -> &'static str {
        match self {
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Power => "**",
            BinaryOperator::Modulo => "%",
            BinaryOperator::Equal => "==",
            BinaryOperator::NotEqual => "!=",
            BinaryOperator::Greater => ">",
            BinaryOperator::GreaterEqual => ">=",
            BinaryOperator::Less => "<",
            BinaryOperator::LessEqual => "<=",
            BinaryOperator::And => "and",
            BinaryOperator::Or => "or",
            BinaryOperator::Is => "is",
            BinaryOperator::In => "in",
            BinaryOperator::BitAnd => "&",
            BinaryOperator::BitOr => "|",
            BinaryOperator::BitXor => "^",
            BinaryOperator::ShiftLeft => "<<",
            BinaryOperator::ShiftRight => ">>",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOperator {
    Assign,
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Modulo,
}

impl AssignOperator {
    /// The binary operator behind a compound assignment (`+=` → `+`).
    pub fn binary(&self) -> Option<BinaryOperator> {
        match self {
            AssignOperator::Assign => None,
            AssignOperator::Add => Some(BinaryOperator::Add),
            AssignOperator::Subtract => Some(BinaryOperator::Subtract),
            AssignOperator::Multiply => Some(BinaryOperator::Multiply),
            AssignOperator::Divide => Some(BinaryOperator::Divide),
            AssignOperator::Power => Some(BinaryOperator::Power),
            AssignOperator::Modulo => Some(BinaryOperator::Modulo),
        }
    }
}

/// Type written in the code: `int`, `string[]`, `Option[int]`, `(int) => int`...
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// Plain or module-qualified name: `int`, `Person`, `math.Point`.
    Named(String),
    /// `Option[int]`, `Result[int, string]`.
    Generic(String, Vec<TypeExpr>),
    Array(Box<TypeExpr>),
    Function {
        params: Vec<TypeExpr>,
        ret: Box<TypeExpr>,
    },
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

/// Right-hand side of a `type` declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef {
    /// `{ Ok(value: string) Err(message: string) }`
    Sum(Vec<Variant>),
    /// `{ name: string  age: int }`
    Shape(Vec<Field>),
    /// `{ [key: string]: string }`
    Index { key: TypeExpr, value: TypeExpr },
    /// `Animal & { breed: string }`: a flat merge of the fields of every part.
    Intersection(Vec<TypeDef>),
    /// `type Age = int`
    Alias(TypeExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeExpr>,
    /// `mut items: int[]`: the function can modify the caller's variable.
    pub is_mut: bool,
}

/// Method or associated function that a call resolves to (`person.greet()`,
/// `Person.create('Ana')`), recorded by the type checker.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodRef {
    /// Module that declares the method.
    pub module: PathBuf,
    /// `Person.greet`
    pub key: String,
    /// Whether the object before the `.` is passed as `self`.
    pub has_self: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportKind {
    /// `import math` / `import math from 'math.ez'`
    Module(String),
    /// `import { add, subtract } from 'math.ez'`
    Names(Vec<String>),
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
    /// `0`, `'yes'`, `true`
    Literal(AstNode),
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
    /// `${value}`; the type checker records the `display` method of its type, if it has one.
    Expression(AstNode, RefCell<Option<MethodRef>>),
}

/// Variant that wraps an error propagated by `?`: `File` in `Err(File(error))`, possibly from
/// an imported module (`config.File`).
#[derive(Debug, Clone, PartialEq)]
pub struct TryWrap {
    pub module: Option<String>,
    pub variant: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectEntry {
    pub key: String,
    pub value: AstNode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    // Expressions
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    TemplateString(Vec<TemplatePart>),
    BooleanLiteral(bool),
    ArrayLiteral(Vec<AstNode>),
    /// `{ name: 'Ana', age: 30 }` or `{ 'hello': 'olá' }`
    ObjectLiteral(Vec<ObjectEntry>),
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
        /// Filled by the type checker when the call is a method or an associated function.
        method: RefCell<Option<MethodRef>>,
        /// For `print` and `string`: the `display` method of each argument's type, if any.
        displays: RefCell<Vec<Option<MethodRef>>>,
    },
    /// `mut items` as a call argument: the callee may modify the variable.
    MutArg(Box<AstNode>),
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
    /// Anonymous function: `(x: int): int => { ... }` or `(x: int) => x * 2`.
    Lambda {
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        body: Rc<AstNode>,
    },
    Block(Vec<AstNode>),

    // Statements
    While {
        condition: Box<AstNode>,
        body: Box<AstNode>,
    },
    /// `for x in items` (`x` is imut) or `for mut x in items`.
    For {
        variable: String,
        is_mut: bool,
        iterable: Box<AstNode>,
        body: Box<AstNode>,
    },
    Break,
    Continue,
    Return(Option<Box<AstNode>>),
    Assign {
        target: Box<AstNode>,
        op: AssignOperator,
        value: Box<AstNode>,
    },
    /// `var` (mutable), `const` (cannot be reassigned, contents can change) or `imut` (nothing
    /// can change).
    VarDecl {
        name: String,
        ty: Option<TypeExpr>,
        value: Box<AstNode>,
        is_const: bool,
        is_imut: bool,
    },
    /// `function add(...)`, or a method/associated function `function Person.greet(self)`,
    /// whose `owner` is `Person`.
    FunctionDecl {
        name: String,
        owner: Option<String>,
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        body: Rc<AstNode>,
    },
    TypeDecl {
        name: String,
        definition: TypeDef,
    },
    Import {
        kind: ImportKind,
        path: String,
    },
    /// `value?`: gives the `Ok`/`Some` value, or returns the `Err`/`None` from the function.
    /// When the error must be wrapped in a variant of the function's error type, the type
    /// checker records that variant in `wrap`.
    Try {
        value: Box<AstNode>,
        wrap: RefCell<Option<TryWrap>>,
    },
    /// `export` before a top-level declaration.
    Export(Box<AstNode>),
    /// Statement annotated with the source line where it starts (used in error messages).
    Located {
        line: usize,
        node: Box<AstNode>,
    },
}

impl AstNode {
    /// Removes the `Located` and `Export` wrappers, returning the declaration itself.
    pub fn unlocated(&self) -> &AstNode {
        match self {
            AstNode::Located { node, .. } | AstNode::Export(node) => node.unlocated(),
            other => other,
        }
    }

    /// Whether this top-level statement is marked with `export`.
    pub fn is_exported(&self) -> bool {
        match self {
            AstNode::Located { node, .. } => node.is_exported(),
            AstNode::Export(_) => true,
            _ => false,
        }
    }

    /// Name declared by a function, variable or type declaration (methods declare no name).
    pub fn declared_name(&self) -> Option<&str> {
        match self.unlocated() {
            AstNode::FunctionDecl {
                name, owner: None, ..
            }
            | AstNode::VarDecl { name, .. }
            | AstNode::TypeDecl { name, .. } => Some(name),
            _ => None,
        }
    }
}

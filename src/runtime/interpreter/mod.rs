//! The interpreter: runs a checked program.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use crate::error::EzError;
use crate::parser::ast::{
    AssignOperator, AstNode, BinaryOperator, ImportKind, MatchArm, MethodRef, Pattern, TemplatePart, TryWrap,
    TypeDef, TypeExpr, UnaryOperator,
};
use crate::runtime::env::Env;
use crate::runtime::stdlib::{self, PowFailure};
use crate::runtime::value::{format_float, Closure, ModuleValue, SumType, Value};
use crate::semantic::builtin_sums::BUILTIN_SUMS;
use crate::semantic::type_checker::{is_stdlib_import, resolve_module_path, Program};

mod modules;
mod statements;
mod expressions;
mod operators;
mod calls;
mod patterns;

/// Non-local control flow while executing statements.
pub enum Flow {
    Break,
    Continue,
    Return(Value),
    Error(EzError),
}

impl From<EzError> for Flow {
    fn from(error: EzError) -> Self {
        Flow::Error(error)
    }
}

pub type Exec<T> = Result<T, Flow>;

/// Where a `mut` argument came from, to store the parameter's final value back.
enum Place {
    Variable(String),
    Field(Value, String),
    Element(Value, Value),
}

/// Argument values and the places of the `mut` ones, by argument position.
type Args = (Vec<Value>, Vec<(usize, Place)>);

pub struct Interpreter {
    program: Program,
    modules: HashMap<PathBuf, Rc<ModuleValue>>,
    stdlib: HashMap<String, Rc<ModuleValue>>,
    prelude: Rc<Env>,
    pub(crate) out: Box<dyn Write>,
    pub(crate) input: Box<dyn BufRead>,
    pub(crate) started: Instant,
    current_module: Option<Rc<ModuleValue>>,
    /// Path of the module being run.
    module_path: PathBuf,
    /// Methods and associated functions, by module and `Type.name`.
    methods: HashMap<(PathBuf, String), Rc<Closure>>,
    /// Functions being executed (0 at the top level of a module).
    function_depth: usize,
    file: Rc<str>,
    dir: PathBuf,
    line: usize,
}

fn is_float_type(ty: &Option<TypeExpr>) -> bool {
    matches!(ty, Some(TypeExpr::Named(name)) if name == "float")
}

/// `int` values stored in `float` variables become floats.
fn coerce(value: Value, to_float: bool) -> Value {
    match value {
        Value::Int(n) if to_float => Value::Float(n as f64),
        other => other,
    }
}

impl Interpreter {
    pub fn new(program: Program, out: Box<dyn Write>, input: Box<dyn BufRead>) -> Self {
        let prelude = Env::new(None);
        for name in ["print", "input", "range", "int", "float", "string", "panic"] {
            prelude.define(name, Value::Builtin(Rc::from(name)));
        }
        for sum in BUILTIN_SUMS.iter().filter(|sum| sum.module.is_none()) {
            let sum = stdlib::sum(sum.name);
            for (variant, _) in &sum.variants {
                prelude.define(variant, sum.constructor(variant));
            }
        }
        Interpreter {
            program,
            modules: HashMap::new(),
            stdlib: HashMap::new(),
            prelude,
            out,
            input,
            started: Instant::now(),
            current_module: None,
            module_path: PathBuf::new(),
            methods: HashMap::new(),
            function_depth: 0,
            file: Rc::from(""),
            dir: PathBuf::from("."),
            line: 0,
        }
    }

    /// Runs the entry module of the program.
    pub fn run(&mut self) -> Result<(), EzError> {
        let entry = self.program.entry.clone();
        let result = self.load_module(&entry).map(|_| ());
        let _ = self.out.flush();
        result
    }

    fn location(&self) -> String {
        if self.line == 0 {
            self.file.to_string()
        } else {
            format!("{}:{}", self.file, self.line)
        }
    }

    pub(crate) fn error(&self, message: impl Into<String>) -> Flow {
        Flow::Error(EzError::RuntimeError(format!("{}: {}", self.location(), message.into())))
    }

}

pub(crate) fn as_float(value: &Value) -> f64 {
    match value {
        Value::Int(n) => *n as f64,
        Value::Float(f) => *f,
        _ => f64::NAN,
    }
}

pub(crate) fn some(value: Value) -> Value {
    stdlib::sum("Option").make("Some", vec![value])
}

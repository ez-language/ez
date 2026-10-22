//! The type checker: checks every module of a program before it runs.

use std::collections::{HashMap, HashSet};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::error::EzError;
use crate::parser::ast::{
    AssignOperator, AstNode, BinaryOperator, Field, ImportKind, MatchArm, MethodRef, Param, Pattern,
    TemplatePart, TryWrap, TypeDef, TypeExpr, UnaryOperator,
};
use crate::parser::parse_source;
use crate::semantic::builtin_sums::{stdlib_functions, BUILTIN_SUMS};
use crate::semantic::types::{ModuleId, Type, TypeId, TypeKind, TypeTable, VariantDef};

mod modules;
mod stdlib;
mod typing;
mod declarations;
mod statements;
mod functions;
mod expressions;
mod calls;
mod patterns;

type CheckResult<T> = Result<T, EzError>;

/// Names of the standard library modules (`import math`).
pub const STDLIB_MODULES: &[&str] = &["time", "file", "datetime", "math", "http", "json", "regex"];

/// A checked program: the parsed modules, ready to be executed.
#[derive(Debug, Clone)]
pub struct Program {
    pub entry: PathBuf,
    pub modules: HashMap<PathBuf, Rc<Vec<AstNode>>>,
}

/// Resolves the path of a user module relative to the directory of the importing file.
pub fn resolve_module_path(dir: &Path, path: &str) -> PathBuf {
    let joined = dir.join(path);
    joined.canonicalize().unwrap_or(joined)
}

pub fn is_stdlib_import(path: &str) -> bool {
    STDLIB_MODULES.contains(&path)
}

#[derive(Debug, Clone)]
enum SymbolKind {
    Value,
    /// Built-in function, identified by its qualified name (`print`, `math.sqrt`).
    Builtin(String),
    /// Variant constructor of a sum type.
    Constructor { id: TypeId, variant: String },
}

#[derive(Debug, Clone)]
struct Symbol {
    ty: Type,
    /// The variable cannot be reassigned (`const`, `imut`, functions...).
    is_const: bool,
    /// Its contents cannot change either (`imut`).
    is_imut: bool,
    kind: SymbolKind,
    /// Module that declared the symbol, when it was imported: it is read-only here.
    owner: Option<String>,
    /// For functions: which parameters are `mut` (empty when none is).
    mut_params: Vec<bool>,
    /// Why the variable is read-only, when the reason is not its declaration (a parameter,
    /// `self`, the `for` variable...).
    reason: Option<&'static str>,
}

impl Symbol {
    fn value(ty: Type, is_const: bool) -> Self {
        Symbol {
            ty,
            is_const,
            is_imut: false,
            kind: SymbolKind::Value,
            owner: None,
            mut_params: Vec::new(),
            reason: None,
        }
    }

    fn builtin(name: &str) -> Self {
        Symbol {
            ty: Type::Void,
            is_const: true,
            is_imut: true,
            kind: SymbolKind::Builtin(name.to_string()),
            owner: None,
            mut_params: Vec::new(),
            reason: None,
        }
    }
}

#[derive(Debug, Clone)]
enum TypeRef {
    Type(Type),
    /// Generic built-in type that needs arguments (`Option`, `Result`).
    Generic(TypeId),
}

#[derive(Debug, Default, Clone)]
struct Scope {
    vars: HashMap<String, Symbol>,
    types: HashMap<String, TypeRef>,
}

#[derive(Debug, Default, Clone)]
struct ModuleType {
    /// How the module is named in messages (`math.ez`, `json`).
    display: String,
    /// Exported values, functions and constructors.
    members: HashMap<String, Symbol>,
    /// Exported types.
    types: HashMap<String, TypeRef>,
    /// Names declared in the module but not exported.
    private: HashSet<String>,
}

/// A method (`function Person.greet(self)`) or an associated function (`function
/// Person.create(name: string)`).
#[derive(Debug, Clone)]
struct MethodInfo {
    /// Full signature; for a method, the first parameter is `self`.
    ty: Type,
    has_self: bool,
    mut_params: Vec<bool>,
    module: PathBuf,
}

struct ReturnContext {
    /// Declared return type; `None` when it must be inferred (anonymous functions).
    declared: Option<Type>,
    inferred: Option<Type>,
}

/// Ids of the types the language and its standard library provide.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinTypes {
    pub option: TypeId,
    pub result: TypeId,
    pub parse_error: TypeId,
    pub math_error: TypeId,
    pub file_error: TypeId,
    pub file_mode: TypeId,
    pub http_error: TypeId,
    pub json: TypeId,
    pub json_object: TypeId,
    pub json_error: TypeId,
    pub date_error: TypeId,
    pub file: TypeId,
    pub datetime: TypeId,
    pub response: TypeId,
    pub regex_match: TypeId,
    pub regex: TypeId,
    pub regex_error: TypeId,
}

/// Per-module checking state.
#[derive(Default)]
struct ModuleState {
    /// Whether this is the main file (where `?` may be used at the top level).
    is_entry: bool,
    scopes: Vec<Scope>,
    /// Index in `scopes` of the parameter scope of each function being checked.
    function_scopes: Vec<usize>,
    returns: Vec<ReturnContext>,
    file: String,
    /// Path of the module being checked (identifies its methods at runtime).
    path: PathBuf,
    dir: PathBuf,
    line: usize,
}

pub struct Checker {
    pub table: TypeTable,
    pub builtins: BuiltinTypes,
    modules: Vec<ModuleType>,
    module_by_path: HashMap<PathBuf, ModuleId>,
    stdlib: HashMap<String, ModuleId>,
    loading: Vec<PathBuf>,
    asts: HashMap<PathBuf, Rc<Vec<AstNode>>>,
    /// Methods and associated functions of each type, by type and name.
    methods: HashMap<(TypeId, String), MethodInfo>,
    /// Argument types of the last built-in call checked.
    last_arg_types: Vec<Type>,
    state: ModuleState,
}

fn is_statement(node: &AstNode) -> bool {
    matches!(
        node.unlocated(),
        AstNode::While { .. }
            | AstNode::For { .. }
            | AstNode::Assign { .. }
            | AstNode::VarDecl { .. }
            | AstNode::FunctionDecl { .. }
            | AstNode::TypeDecl { .. }
            | AstNode::Import { .. }
    )
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    /// Checks the file at `path` and every module it imports.
    pub fn check_file(mut self, path: &Path) -> CheckResult<Program> {
        let source = std::fs::read_to_string(path).map_err(|e| {
            EzError::ImportError(format!("cannot read '{}': {}", path.display(), e))
        })?;
        let entry = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.check_module(&entry, &path.display().to_string(), &source)?;
        Ok(Program {
            entry,
            modules: self.asts,
        })
    }

    /// Checks source code as if it were the file `path` (imports are relative to its directory).
    pub fn check_source(mut self, path: &Path, source: &str) -> CheckResult<Program> {
        let entry = path.to_path_buf();
        self.check_module(&entry, &path.display().to_string(), source)?;
        Ok(Program {
            entry,
            modules: self.asts,
        })
    }

    fn err<T>(&self, message: String) -> CheckResult<T> {
        let location = if self.state.line == 0 {
            self.state.file.clone()
        } else {
            format!("{}:{}", self.state.file, self.state.line)
        };
        Err(EzError::TypeError(format!("{}: {}", location, message)))
    }

    fn show(&self, ty: &Type) -> String {
        self.table.display(ty)
    }

    fn push_scope(&mut self) {
        self.state.scopes.push(Scope::default());
    }

    fn pop_scope(&mut self) {
        self.state.scopes.pop();
    }

    fn current_scope(&mut self) -> &mut Scope {
        self.state.scopes.last_mut().expect("scope stack is never empty")
    }

    fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.state
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.vars.get(name))
    }

    /// A function captures a read-only copy of the variables of the functions around it: they
    /// cannot be assigned (module variables are not captured, they belong to the module).
    fn ensure_not_captured(&self, name: &str) -> CheckResult<()> {
        let boundary = match self.state.function_scopes.last() {
            Some(boundary) => *boundary,
            None => return Ok(()),
        };
        let index = self
            .state
            .scopes
            .iter()
            .rposition(|scope| scope.vars.contains_key(name));
        match index {
            // 0 is the prelude and 1 the module scope.
            Some(index) if index > 1 && index < boundary => self.err(format!(
                "cannot modify '{}' here: functions capture a read-only copy of the variables around them",
                name
            )),
            _ => Ok(()),
        }
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        self.state
            .scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.vars.get_mut(name))
    }

    fn lookup_type(&self, name: &str) -> Option<&TypeRef> {
        self.state
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.types.get(name))
    }

    fn declare(&mut self, name: &str, symbol: Symbol) -> CheckResult<()> {
        if self.current_scope().vars.contains_key(name) {
            return self.err(format!("'{}' is already declared in this scope", name));
        }
        self.current_scope().vars.insert(name.to_string(), symbol);
        Ok(())
    }

    fn option_of(&self, ty: Type) -> Type {
        Type::Named {
            id: self.builtins.option,
            args: vec![ty],
        }
    }

    fn named(id: TypeId) -> Type {
        Type::Named { id, args: vec![] }
    }


}

/// Turns `line N: message` into `file:N: message`.
fn prefix_error(file: &str, error: EzError) -> EzError {
    let locate = |message: String| -> String {
        if let Some(rest) = message.strip_prefix("line ") {
            if let Some((line, text)) = rest.split_once(": ") {
                if line.chars().all(|c| c.is_ascii_digit()) {
                    return format!("{}:{}: {}", file, line, text);
                }
            }
        }
        format!("{}: {}", file, message)
    };
    match error {
        EzError::LexerError(message) => EzError::LexerError(locate(message)),
        EzError::ParserError(message) => EzError::ParserError(locate(message)),
        other => other,
    }
}

/// Checks the file at `path` (and its imports).
pub fn check_types(path: &Path) -> Result<Program, EzError> {
    Checker::new().check_file(path)
}

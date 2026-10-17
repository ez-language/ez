use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::error::EzError;
use crate::parser::ast::{
    AssignOperator, AstNode, BinaryOperator, Field, ImportKind, MatchArm, Param, Pattern,
    TemplatePart, TypeDef, TypeExpr, UnaryOperator,
};
use crate::parser::parse_source;
use crate::semantic::types::{ModuleId, Type, TypeId, TypeKind, TypeTable, VariantDef};

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
    is_const: bool,
    kind: SymbolKind,
}

impl Symbol {
    fn value(ty: Type, is_const: bool) -> Self {
        Symbol {
            ty,
            is_const,
            kind: SymbolKind::Value,
        }
    }

    fn builtin(name: &str) -> Self {
        Symbol {
            ty: Type::Void,
            is_const: true,
            kind: SymbolKind::Builtin(name.to_string()),
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
    members: HashMap<String, Symbol>,
    types: HashMap<String, TypeRef>,
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
    pub file: TypeId,
    pub datetime: TypeId,
    pub response: TypeId,
    pub regex_match: TypeId,
}

/// Per-module checking state.
#[derive(Default)]
struct ModuleState {
    scopes: Vec<Scope>,
    returns: Vec<ReturnContext>,
    file: String,
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
            | AstNode::InterfaceDecl { .. }
            | AstNode::Import { .. }
    )
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    pub fn new() -> Self {
        let mut table = TypeTable::default();
        let option = table.add(
            "Option",
            1,
            TypeKind::Sum(vec![
                VariantDef {
                    name: "Some".to_string(),
                    fields: vec![("value".to_string(), Type::Param(0))],
                },
                VariantDef {
                    name: "None".to_string(),
                    fields: vec![],
                },
            ]),
        );
        let result = table.add(
            "Result",
            2,
            TypeKind::Sum(vec![
                VariantDef {
                    name: "Ok".to_string(),
                    fields: vec![("value".to_string(), Type::Param(0))],
                },
                VariantDef {
                    name: "Err".to_string(),
                    fields: vec![("error".to_string(), Type::Param(1))],
                },
            ]),
        );
        let file = table.add(
            "File",
            0,
            TypeKind::Builtin {
                fields: vec![],
                methods: vec![
                    ("read".to_string(), vec![], Type::String),
                    ("write".to_string(), vec![Type::String], Type::Void),
                    ("close".to_string(), vec![], Type::Void),
                ],
            },
        );
        let int_fields = |names: &[&str]| -> Vec<(String, Type)> {
            names.iter().map(|n| (n.to_string(), Type::Int)).collect()
        };
        let datetime = table.add(
            "DateTime",
            0,
            TypeKind::Builtin {
                fields: int_fields(&["year", "month", "day", "hour", "minute", "second"]),
                methods: vec![],
            },
        );
        let response = table.add(
            "Response",
            0,
            TypeKind::Builtin {
                fields: vec![
                    ("status".to_string(), Type::Int),
                    ("body".to_string(), Type::String),
                ],
                methods: vec![],
            },
        );
        let regex_match = table.add(
            "Match",
            0,
            TypeKind::Builtin {
                fields: vec![
                    ("value".to_string(), Type::String),
                    ("start".to_string(), Type::Int),
                    ("end".to_string(), Type::Int),
                ],
                methods: vec![],
            },
        );

        Checker {
            table,
            builtins: BuiltinTypes {
                option,
                result,
                file,
                datetime,
                response,
                regex_match,
            },
            modules: Vec::new(),
            module_by_path: HashMap::new(),
            stdlib: HashMap::new(),
            loading: Vec::new(),
            asts: HashMap::new(),
            state: ModuleState::default(),
        }
    }

    /// Scope with the built-in functions, types and constructors.
    fn prelude(&self) -> Scope {
        let mut scope = Scope::default();
        for name in ["print", "input", "range", "int", "float", "string"] {
            scope.vars.insert(name.to_string(), Symbol::builtin(name));
        }
        let primitives = [
            ("int", Type::Int),
            ("float", Type::Float),
            ("string", Type::String),
            ("boolean", Type::Boolean),
            ("bool", Type::Boolean),
            ("void", Type::Void),
            ("any", Type::Any),
        ];
        for (name, ty) in primitives {
            scope.types.insert(name.to_string(), TypeRef::Type(ty));
        }
        scope
            .types
            .insert("Option".to_string(), TypeRef::Generic(self.builtins.option));
        scope
            .types
            .insert("Result".to_string(), TypeRef::Generic(self.builtins.result));
        for id in [self.builtins.option, self.builtins.result] {
            self.declare_constructors_into(&mut scope, id);
        }
        scope
    }

    fn declare_constructors_into(&self, scope: &mut Scope, id: TypeId) {
        let info = self.table.get(id);
        let params = info.params;
        if let TypeKind::Sum(variants) = &info.kind {
            for variant in variants {
                let result = Type::Named {
                    id,
                    args: vec![Type::Unknown; params],
                };
                let ty = if variant.fields.is_empty() {
                    result
                } else {
                    Type::Function {
                        params: variant.fields.iter().map(|(_, t)| t.clone()).collect(),
                        ret: Box::new(result),
                    }
                };
                scope.vars.insert(
                    variant.name.clone(),
                    Symbol {
                        ty,
                        is_const: true,
                        kind: SymbolKind::Constructor {
                            id,
                            variant: variant.name.clone(),
                        },
                    },
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // Entry points
    // ------------------------------------------------------------------

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

    fn check_module(&mut self, path: &Path, display: &str, source: &str) -> CheckResult<ModuleId> {
        let program = parse_source(source).map_err(|e| prefix_error(display, e))?;
        let program = Rc::new(program);
        self.asts.insert(path.to_path_buf(), Rc::clone(&program));
        self.loading.push(path.to_path_buf());

        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let prelude = self.prelude();
        let saved = std::mem::replace(
            &mut self.state,
            ModuleState {
                scopes: vec![prelude, Scope::default()],
                returns: Vec::new(),
                file: display.to_string(),
                dir,
                line: 0,
            },
        );

        let result = self.check_statements(&program, None);
        let module_scope = self.state.scopes.get(1).cloned().unwrap_or_default();
        self.state = saved;
        self.loading.pop();
        result?;

        self.modules.push(ModuleType {
            members: module_scope.vars,
            types: module_scope.types,
        });
        let id = self.modules.len() - 1;
        self.module_by_path.insert(path.to_path_buf(), id);
        Ok(id)
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

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

    fn result_of(&self, ok: Type, err: Type) -> Type {
        Type::Named {
            id: self.builtins.result,
            args: vec![ok, err],
        }
    }

    fn named(id: TypeId) -> Type {
        Type::Named { id, args: vec![] }
    }

    // ------------------------------------------------------------------
    // Type compatibility
    // ------------------------------------------------------------------

    /// Whether a value of type `actual` can be used where `expected` is required.
    pub fn assignable(&self, expected: &Type, actual: &Type) -> bool {
        match (expected, actual) {
            (_, Type::Never | Type::Unknown | Type::Any) | (Type::Any | Type::Unknown, _) => true,
            (Type::Float, Type::Int) => true,
            (Type::List(e), Type::List(a)) => self.assignable(e, a),
            (Type::Named { id: e, args: ea }, Type::Named { id: a, args: aa })
            | (Type::Named { id: e, args: ea }, Type::Variant { id: a, args: aa, .. })
                if e == a =>
            {
                ea.iter().zip(aa).all(|(x, y)| self.assignable(x, y))
            }
            (
                Type::Variant {
                    id: e,
                    args: ea,
                    variant: ev,
                },
                Type::Variant {
                    id: a,
                    args: aa,
                    variant: av,
                },
            ) => e == a && ev == av && ea.iter().zip(aa).all(|(x, y)| self.assignable(x, y)),
            (Type::Named { id, .. }, actual) => match &self.table.get(*id).kind {
                // Interfaces are structural: any value with the same shape is accepted.
                TypeKind::Interface(fields) => match self.table.fields_of(actual) {
                    Some(actual_fields) => self.same_fields(fields, &actual_fields),
                    None => false,
                },
                TypeKind::Shape(fields) => match actual {
                    Type::Record(actual_fields) => self.same_fields(fields, actual_fields),
                    _ => false,
                },
                TypeKind::Dict { value, .. } => match actual {
                    Type::Record(actual_fields) => {
                        actual_fields.iter().all(|(_, t)| self.assignable(value, t))
                    }
                    _ => false,
                },
                _ => false,
            },
            (Type::Record(fields), actual) => match self.table.fields_of(actual) {
                Some(actual_fields) => self.same_fields(fields, &actual_fields),
                None => false,
            },
            (
                Type::Function { params: ep, ret: er },
                Type::Function { params: ap, ret: ar },
            ) => {
                ep.len() == ap.len()
                    && ep.iter().zip(ap).all(|(e, a)| self.assignable(a, e))
                    && self.assignable(er, ar)
            }
            (e, a) => e == a,
        }
    }

    /// Same set of field names, with compatible types.
    fn same_fields(&self, expected: &[(String, Type)], actual: &[(String, Type)]) -> bool {
        expected.len() == actual.len()
            && expected.iter().all(|(name, ty)| {
                actual
                    .iter()
                    .any(|(n, t)| n == name && self.assignable(ty, t))
            })
    }

    /// Common type of two branches, if there is one.
    fn join(&self, a: &Type, b: &Type) -> Option<Type> {
        match (a, b) {
            _ if a == b => Some(a.clone()),
            (Type::Never, other) | (other, Type::Never) => Some(other.clone()),
            (Type::Unknown, other) | (other, Type::Unknown) => Some(other.clone()),
            (Type::Any, _) | (_, Type::Any) => Some(Type::Any),
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => Some(Type::Float),
            (Type::List(x), Type::List(y)) => Some(Type::List(Box::new(self.join(x, y)?))),
            (
                Type::Named { id: x, args: xa } | Type::Variant { id: x, args: xa, .. },
                Type::Named { id: y, args: ya } | Type::Variant { id: y, args: ya, .. },
            ) if x == y => {
                let args = xa
                    .iter()
                    .zip(ya)
                    .map(|(p, q)| self.join(p, q))
                    .collect::<Option<Vec<_>>>()?;
                Some(Type::Named { id: *x, args })
            }
            _ if self.assignable(a, b) => Some(a.clone()),
            _ if self.assignable(b, a) => Some(b.clone()),
            _ => None,
        }
    }

    // ------------------------------------------------------------------
    // Type expressions and declarations
    // ------------------------------------------------------------------

    fn resolve_type(&self, expr: &TypeExpr) -> CheckResult<Type> {
        match expr {
            TypeExpr::Named(name) => match self.find_type(name)? {
                TypeRef::Type(ty) => Ok(ty),
                TypeRef::Generic(id) => {
                    let info = self.table.get(id);
                    self.err(format!(
                        "type '{}' needs {} type argument(s), e.g. {}[int]",
                        info.name, info.params, info.name
                    ))
                }
            },
            TypeExpr::Generic(name, args) => match self.find_type(name)? {
                TypeRef::Generic(id) => {
                    let expected = self.table.get(id).params;
                    if expected != args.len() {
                        return self.err(format!(
                            "type '{}' expects {} type argument(s), found {}",
                            name,
                            expected,
                            args.len()
                        ));
                    }
                    let args = args
                        .iter()
                        .map(|a| self.resolve_type(a))
                        .collect::<CheckResult<Vec<_>>>()?;
                    Ok(Type::Named { id, args })
                }
                TypeRef::Type(_) => self.err(format!("type '{}' is not generic", name)),
            },
            TypeExpr::Array(inner) => Ok(Type::List(Box::new(self.resolve_type(inner)?))),
            TypeExpr::Function { params, ret } => Ok(Type::Function {
                params: params
                    .iter()
                    .map(|p| self.resolve_type(p))
                    .collect::<CheckResult<Vec<_>>>()?,
                ret: Box::new(self.resolve_type(ret)?),
            }),
        }
    }

    fn find_type(&self, name: &str) -> CheckResult<TypeRef> {
        if let Some((module, type_name)) = name.split_once('.') {
            let module_id = match self.lookup(module).map(|s| &s.ty) {
                Some(Type::Module(id)) => *id,
                _ => return self.err(format!("'{}' is not an imported module", module)),
            };
            return match self.modules[module_id].types.get(type_name) {
                Some(found) => Ok(found.clone()),
                None => self.err(format!("module '{}' has no type '{}'", module, type_name)),
            };
        }
        match self.lookup_type(name) {
            Some(found) => Ok(found.clone()),
            None => self.err(format!("unknown type '{}'", name)),
        }
    }

    fn resolve_fields(&self, fields: &[Field]) -> CheckResult<Vec<(String, Type)>> {
        fields
            .iter()
            .map(|f| Ok((f.name.clone(), self.resolve_type(&f.ty)?)))
            .collect()
    }

    /// Declares the types, constructors and function signatures of a block before checking it,
    /// so they can be used before their definition (and recursively).
    fn hoist(&mut self, statements: &[AstNode]) -> CheckResult<()> {
        let mut pending: Vec<(&AstNode, Option<TypeId>)> = Vec::new();
        for statement in statements {
            self.set_line(statement);
            match statement.unlocated() {
                AstNode::TypeDecl {
                    name,
                    definition: TypeDef::Alias(_),
                } => {
                    self.check_type_name_free(name)?;
                    pending.push((statement, None));
                }
                AstNode::TypeDecl { name, .. } | AstNode::InterfaceDecl { name, .. } => {
                    self.check_type_name_free(name)?;
                    let id = self.table.add(name, 0, TypeKind::Pending);
                    self.current_scope()
                        .types
                        .insert(name.clone(), TypeRef::Type(Self::named(id)));
                    pending.push((statement, Some(id)));
                }
                _ => {}
            }
        }

        for (statement, id) in pending {
            self.set_line(statement);
            match (statement.unlocated(), id) {
                (
                    AstNode::TypeDecl {
                        name,
                        definition: TypeDef::Alias(expr),
                    },
                    None,
                ) => {
                    let ty = self.resolve_type(expr)?;
                    self.current_scope()
                        .types
                        .insert(name.clone(), TypeRef::Type(ty));
                }
                (AstNode::TypeDecl { name, definition }, Some(id)) => {
                    let kind = self.resolve_type_def(name, definition)?;
                    self.table.types[id].kind = kind;
                    if matches!(self.table.get(id).kind, TypeKind::Sum(_)) {
                        let mut scope = std::mem::take(self.current_scope());
                        self.declare_constructors_into(&mut scope, id);
                        *self.current_scope() = scope;
                    }
                }
                (AstNode::InterfaceDecl { fields, .. }, Some(id)) => {
                    let fields = self.resolve_fields(fields)?;
                    self.table.types[id].kind = TypeKind::Interface(fields);
                }
                _ => {}
            }
        }

        for statement in statements {
            self.set_line(statement);
            if let AstNode::FunctionDecl {
                name,
                params,
                return_type,
                ..
            } = statement.unlocated()
            {
                let ty = self.function_signature(name, params, return_type.as_ref(), true)?;
                self.declare(name, Symbol::value(ty, true))?;
            }
        }
        Ok(())
    }

    fn check_type_name_free(&mut self, name: &str) -> CheckResult<()> {
        if self.current_scope().types.contains_key(name) {
            return self.err(format!("type '{}' is already declared in this scope", name));
        }
        Ok(())
    }

    fn resolve_type_def(&self, name: &str, definition: &TypeDef) -> CheckResult<TypeKind> {
        match definition {
            TypeDef::Sum(variants) => {
                let mut defs = Vec::new();
                for variant in variants {
                    defs.push(VariantDef {
                        name: variant.name.clone(),
                        fields: self.resolve_fields(&variant.fields)?,
                    });
                }
                Ok(TypeKind::Sum(defs))
            }
            TypeDef::Shape(fields) => Ok(TypeKind::Shape(self.resolve_fields(fields)?)),
            TypeDef::Index { key, value } => {
                let key = self.resolve_type(key)?;
                if key != Type::String {
                    return self.err(format!(
                        "index signature keys must be string, found {}",
                        self.show(&key)
                    ));
                }
                Ok(TypeKind::Dict {
                    key,
                    value: self.resolve_type(value)?,
                })
            }
            TypeDef::Alias(expr) => {
                // Only reached inside an intersection.
                let ty = self.resolve_type(expr)?;
                match self.table.fields_of(&ty) {
                    Some(fields) => Ok(TypeKind::Shape(fields)),
                    None => self.err(format!(
                        "'{}' cannot be part of the intersection '{}': only types with fields can be merged",
                        self.show(&ty),
                        name
                    )),
                }
            }
            TypeDef::Intersection(parts) => {
                let mut merged: Vec<(String, Type)> = Vec::new();
                for part in parts {
                    let fields = match self.resolve_type_def(name, part)? {
                        TypeKind::Shape(fields) | TypeKind::Interface(fields) => fields,
                        _ => {
                            return self.err(format!(
                                "only types with fields can be merged with '&' (in '{}')",
                                name
                            ))
                        }
                    };
                    for (field, ty) in fields {
                        match merged.iter().find(|(n, _)| *n == field) {
                            Some((_, existing)) if *existing != ty => {
                                return self.err(format!(
                                    "field '{}' has conflicting types in '{}': {} and {}",
                                    field,
                                    name,
                                    self.show(existing),
                                    self.show(&ty)
                                ));
                            }
                            Some(_) => {}
                            None => merged.push((field, ty)),
                        }
                    }
                }
                Ok(TypeKind::Shape(merged))
            }
        }
    }

    fn function_signature(
        &self,
        name: &str,
        params: &[Param],
        return_type: Option<&TypeExpr>,
        is_declaration: bool,
    ) -> CheckResult<Type> {
        let mut param_types = Vec::new();
        for param in params {
            match &param.ty {
                Some(ty) => param_types.push(self.resolve_type(ty)?),
                None => {
                    return self.err(format!(
                        "parameter '{}' of '{}' needs a type annotation",
                        param.name, name
                    ))
                }
            }
        }
        let ret = match return_type {
            Some(ty) => self.resolve_type(ty)?,
            None if is_declaration => Type::Void,
            None => Type::Unknown,
        };
        Ok(Type::Function {
            params: param_types,
            ret: Box::new(ret),
        })
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    fn set_line(&mut self, node: &AstNode) {
        if let AstNode::Located { line, .. } = node {
            self.state.line = *line;
        }
    }

    /// Checks a sequence of statements in the current scope; returns the type of its value (the
    /// last statement, when it is an expression).
    fn check_statements(&mut self, statements: &[AstNode], expected: Option<&Type>) -> CheckResult<Type> {
        self.hoist(statements)?;
        let mut tail = Type::Void;
        for (i, statement) in statements.iter().enumerate() {
            let is_last = i + 1 == statements.len();
            tail = self.check_statement(statement, if is_last { expected } else { None })?;
            if !is_last && !matches!(tail, Type::Never) {
                tail = Type::Void;
            }
        }
        if let Some(last) = statements.last() {
            if is_statement(last) {
                return Ok(Type::Void);
            }
        }
        Ok(tail)
    }

    fn check_block(
        &mut self,
        node: &AstNode,
        expected: Option<&Type>,
        bindings: Vec<(String, Symbol)>,
    ) -> CheckResult<Type> {
        self.push_scope();
        for (name, symbol) in bindings {
            self.current_scope().vars.insert(name, symbol);
        }
        let result = match node.unlocated() {
            AstNode::Block(statements) => self.check_statements(statements, expected),
            other => self.check_statement(other, expected),
        };
        self.pop_scope();
        result
    }

    fn check_statement(&mut self, node: &AstNode, expected: Option<&Type>) -> CheckResult<Type> {
        match node {
            AstNode::Located { line, node } => {
                self.state.line = *line;
                self.check_statement(node, expected)
            }
            AstNode::VarDecl {
                name,
                ty,
                value,
                is_const,
            } => {
                let declared = match ty {
                    Some(ty) => Some(self.resolve_type(ty)?),
                    None => None,
                };
                let value_ty = self.check_value(value, declared.as_ref())?;
                let var_ty = match declared {
                    Some(declared) => {
                        if !self.assignable(&declared, &value_ty) {
                            return self.err(format!(
                                "cannot assign a value of type {} to '{}' of type {}",
                                self.show(&value_ty),
                                name,
                                self.show(&declared)
                            ));
                        }
                        declared
                    }
                    None => value_ty,
                };
                self.declare(name, Symbol::value(var_ty, *is_const))?;
                Ok(Type::Void)
            }
            AstNode::Assign { target, op, value } => {
                self.check_assign(target, op, value)?;
                Ok(Type::Void)
            }
            AstNode::FunctionDecl {
                name,
                params,
                return_type,
                body,
            } => {
                let signature = match self.state.scopes.last().and_then(|s| s.vars.get(name)) {
                    Some(symbol) => symbol.ty.clone(),
                    None => self.function_signature(name, params, return_type.as_ref(), true)?,
                };
                self.check_function(name, params, signature, body)?;
                Ok(Type::Void)
            }
            AstNode::TypeDecl { .. } | AstNode::InterfaceDecl { .. } => Ok(Type::Void),
            AstNode::Import { kind, path } => {
                self.check_import(kind, path)?;
                Ok(Type::Void)
            }
            AstNode::While { condition, body } => {
                self.check_condition(condition, "while")?;
                self.check_block(body, None, vec![])?;
                Ok(Type::Void)
            }
            AstNode::For {
                variable,
                iterable,
                body,
            } => {
                let iterable_ty = self.check_value(iterable, None)?;
                let item = match &iterable_ty {
                    Type::List(inner) => (**inner).clone(),
                    Type::String => Type::String,
                    Type::Any | Type::Unknown => Type::Any,
                    Type::Named { id, .. } => match &self.table.get(*id).kind {
                        TypeKind::Dict { key, .. } => key.clone(),
                        _ => return self.not_iterable(&iterable_ty),
                    },
                    _ => return self.not_iterable(&iterable_ty),
                };
                self.check_block(
                    body,
                    None,
                    vec![(variable.clone(), Symbol::value(item, false))],
                )?;
                Ok(Type::Void)
            }
            AstNode::Break | AstNode::Continue => Ok(Type::Never),
            AstNode::Return(value) => {
                self.check_return(value.as_deref())?;
                Ok(Type::Never)
            }
            other => self.check_expr(other, expected),
        }
    }

    fn not_iterable<T>(&self, ty: &Type) -> CheckResult<T> {
        self.err(format!(
            "cannot iterate over a value of type {} (use a list, a string or range())",
            self.show(ty)
        ))
    }

    fn check_condition(&mut self, condition: &AstNode, keyword: &str) -> CheckResult<()> {
        let ty = self.check_value(condition, Some(&Type::Boolean))?;
        if !matches!(ty, Type::Boolean | Type::Any | Type::Never) {
            return self.err(format!(
                "the condition of '{}' must be boolean, found {}",
                keyword,
                self.show(&ty)
            ));
        }
        Ok(())
    }

    fn check_return(&mut self, value: Option<&AstNode>) -> CheckResult<()> {
        let declared = match self.state.returns.last() {
            Some(context) => context.declared.clone(),
            None => return self.err("'return' outside of a function".to_string()),
        };
        let value_ty = match value {
            Some(value) => Some(self.check_value(value, declared.as_ref())?),
            None => None,
        };
        match (declared, value_ty) {
            (Some(Type::Void), Some(_)) => {
                self.err("cannot return a value from a function without a return type".to_string())
            }
            (Some(Type::Void), None) => Ok(()),
            (Some(declared), None) => self.err(format!(
                "this function must return a value of type {}",
                self.show(&declared)
            )),
            (Some(declared), Some(actual)) => {
                if self.assignable(&declared, &actual) {
                    Ok(())
                } else {
                    self.err(format!(
                        "returning {}, but the function returns {}",
                        self.show(&actual),
                        self.show(&declared)
                    ))
                }
            }
            (None, actual) => {
                let actual = actual.unwrap_or(Type::Void);
                let previous = self.state.returns.last().and_then(|c| c.inferred.clone());
                let inferred = match previous {
                    None => actual,
                    Some(previous) => match self.join(&previous, &actual) {
                        Some(joined) => joined,
                        None => {
                            return self.err(format!(
                                "inconsistent return types: {} and {}",
                                self.show(&previous),
                                self.show(&actual)
                            ))
                        }
                    },
                };
                if let Some(context) = self.state.returns.last_mut() {
                    context.inferred = Some(inferred);
                }
                Ok(())
            }
        }
    }

    fn check_assign(&mut self, target: &AstNode, op: &AssignOperator, value: &AstNode) -> CheckResult<()> {
        match target {
            AstNode::Identifier(name) => {
                let existing = self.lookup(name).cloned();
                let symbol = match existing {
                    Some(symbol) => symbol,
                    None => {
                        if *op != AssignOperator::Assign {
                            return self.err(format!("undefined variable '{}'", name));
                        }
                        // Assigning to a new name declares a mutable variable.
                        let ty = self.check_value(value, None)?;
                        return self.declare(name, Symbol::value(ty, false));
                    }
                };
                if !matches!(symbol.kind, SymbolKind::Value) || matches!(symbol.ty, Type::Module(_)) {
                    return self.err(format!("cannot assign to '{}'", name));
                }
                if symbol.is_const {
                    return self.err(format!("cannot modify a 'const' value ('{}')", name));
                }
                let new_ty = self.check_assigned_value(&symbol.ty, op, value, name)?;
                if symbol.ty.contains_unknown() {
                    let refined = self.join(&symbol.ty, &new_ty).unwrap_or(symbol.ty.clone());
                    if let Some(symbol) = self.lookup_mut(name) {
                        symbol.ty = refined;
                    }
                }
                Ok(())
            }
            AstNode::Index { target: inner, index } => {
                let container = self.check_value(inner, None)?;
                let index_ty = self.check_value(index, None)?;
                let element = match &container {
                    Type::List(element) => {
                        self.expect_type(&Type::Int, &index_ty, "list index")?;
                        (**element).clone()
                    }
                    Type::Any | Type::Unknown => Type::Any,
                    Type::Named { id, .. } => match &self.table.get(*id).kind {
                        TypeKind::Dict { key, value } => {
                            let (key, value) = (key.clone(), value.clone());
                            self.expect_type(&key, &index_ty, "dictionary key")?;
                            value
                        }
                        _ => return self.err(format!("cannot assign by index to {}", self.show(&container))),
                    },
                    Type::String => return self.err("strings cannot be modified".to_string()),
                    other => return self.err(format!("cannot assign by index to {}", self.show(other))),
                };
                self.check_assigned_value(&element, op, value, "element")?;
                Ok(())
            }
            AstNode::Member { object, name } => {
                let object_ty = self.check_value(object, None)?;
                let field_ty = match &object_ty {
                    Type::Any | Type::Unknown => Type::Any,
                    Type::Module(_) => return self.err(format!("cannot assign to module member '{}'", name)),
                    Type::Variant { .. } => {
                        return self.err(format!("variant fields cannot be modified ('{}')", name))
                    }
                    Type::Named { id, .. }
                        if matches!(self.table.get(*id).kind, TypeKind::Builtin { .. }) =>
                    {
                        return self.err(format!("'{}' is read-only", name))
                    }
                    other => match self.table.fields_of(other) {
                        Some(fields) => match fields.into_iter().find(|(n, _)| n == name) {
                            Some((_, ty)) => ty,
                            None => {
                                return self.err(format!(
                                    "{} has no field '{}'",
                                    self.show(other),
                                    name
                                ))
                            }
                        },
                        None => {
                            return self.err(format!(
                                "cannot assign field '{}' on {}",
                                name,
                                self.show(other)
                            ))
                        }
                    },
                };
                self.check_assigned_value(&field_ty, op, value, name)?;
                Ok(())
            }
            _ => self.err("invalid assignment target".to_string()),
        }
    }

    /// Checks `target op= value` where the target has type `target_ty`; returns the value type.
    fn check_assigned_value(
        &mut self,
        target_ty: &Type,
        op: &AssignOperator,
        value: &AstNode,
        name: &str,
    ) -> CheckResult<Type> {
        let value_ty = match op.binary() {
            None => self.check_value(value, Some(target_ty))?,
            Some(binary) => {
                let right = self.check_value(value, None)?;
                self.binary_type(binary, target_ty, &right)?
            }
        };
        if !self.assignable(target_ty, &value_ty) {
            return self.err(format!(
                "cannot assign a value of type {} to '{}' of type {}",
                self.show(&value_ty),
                name,
                self.show(target_ty)
            ));
        }
        Ok(value_ty)
    }

    fn expect_type(&self, expected: &Type, actual: &Type, what: &str) -> CheckResult<()> {
        if self.assignable(expected, actual) {
            Ok(())
        } else {
            self.err(format!(
                "{} must be {}, found {}",
                what,
                self.show(expected),
                self.show(actual)
            ))
        }
    }

    fn check_import(&mut self, kind: &ImportKind, path: &str) -> CheckResult<()> {
        let module_id = if is_stdlib_import(path) {
            self.stdlib_module(path)
        } else {
            let resolved = resolve_module_path(&self.state.dir, path);
            match self.module_by_path.get(&resolved) {
                Some(id) => *id,
                None => {
                    if self.loading.contains(&resolved) {
                        return self.err(format!("circular import of '{}'", path));
                    }
                    let source = match std::fs::read_to_string(&resolved) {
                        Ok(source) => source,
                        Err(e) => return self.err(format!("cannot import '{}': {}", path, e)),
                    };
                    let display = resolved.display().to_string();
                    self.check_module(&resolved, &display, &source)?
                }
            }
        };

        match kind {
            ImportKind::Module(name) => {
                self.declare(name, Symbol::value(Type::Module(module_id), true))?;
            }
            ImportKind::Names(names) => {
                let module = self.modules[module_id].clone();
                for name in names {
                    let mut found = false;
                    if let Some(symbol) = module.members.get(name) {
                        self.declare(name, symbol.clone())?;
                        found = true;
                    }
                    if let Some(type_ref) = module.types.get(name) {
                        self.current_scope().types.insert(name.clone(), type_ref.clone());
                        // Importing a sum type also imports its constructors.
                        if let TypeRef::Type(Type::Named { id, .. }) = type_ref {
                            if self.table.variants_of(*id).is_some() {
                                let mut scope = std::mem::take(self.current_scope());
                                self.declare_constructors_into(&mut scope, *id);
                                *self.current_scope() = scope;
                            }
                        }
                        found = true;
                    }
                    if !found {
                        return self.err(format!("module '{}' has no member '{}'", path, name));
                    }
                }
            }
        }
        Ok(())
    }

    fn stdlib_module(&mut self, name: &str) -> ModuleId {
        if let Some(id) = self.stdlib.get(name) {
            return *id;
        }
        let functions: &[&str] = match name {
            "time" => &["sleep", "now", "monotonic"],
            "file" => &["open"],
            "datetime" => &["now", "today", "format", "parse"],
            "math" => &["abs", "sqrt", "pow", "floor", "ceil", "round"],
            "http" => &["get", "post", "put", "delete"],
            "json" => &["parse", "stringify"],
            "regex" => &["match", "search", "findAll", "replace"],
            _ => &[],
        };
        let mut module = ModuleType::default();
        for function in functions {
            module
                .members
                .insert(function.to_string(), Symbol::builtin(&format!("{}.{}", name, function)));
        }
        let b = self.builtins;
        match name {
            "math" => {
                module.members.insert("pi".to_string(), Symbol::value(Type::Float, true));
                module.members.insert("e".to_string(), Symbol::value(Type::Float, true));
            }
            "file" => {
                module.types.insert("File".to_string(), TypeRef::Type(Self::named(b.file)));
            }
            "datetime" => {
                module
                    .types
                    .insert("DateTime".to_string(), TypeRef::Type(Self::named(b.datetime)));
            }
            "http" => {
                module
                    .types
                    .insert("Response".to_string(), TypeRef::Type(Self::named(b.response)));
            }
            "regex" => {
                module
                    .types
                    .insert("Match".to_string(), TypeRef::Type(Self::named(b.regex_match)));
            }
            _ => {}
        }
        self.modules.push(module);
        let id = self.modules.len() - 1;
        self.stdlib.insert(name.to_string(), id);
        id
    }

    // ------------------------------------------------------------------
    // Functions
    // ------------------------------------------------------------------

    fn check_function(
        &mut self,
        name: &str,
        params: &[Param],
        signature: Type,
        body: &AstNode,
    ) -> CheckResult<Type> {
        let (param_types, declared_ret) = match signature {
            Type::Function { params, ret } => (params, *ret),
            _ => unreachable!("function signature is always a function type"),
        };
        let declared = if declared_ret == Type::Unknown {
            None
        } else {
            Some(declared_ret.clone())
        };

        self.push_scope();
        for (param, ty) in params.iter().zip(&param_types) {
            self.declare(&param.name, Symbol::value(ty.clone(), false))?;
        }
        self.state.returns.push(ReturnContext {
            declared: declared.clone(),
            inferred: None,
        });

        let value_expected = declared.as_ref().filter(|t| **t != Type::Void);
        let body_result = match body.unlocated() {
            AstNode::Block(statements) => self.check_statements(statements, value_expected),
            expression => self.check_value(expression, value_expected),
        };
        let context = self.state.returns.pop();
        self.pop_scope();
        let body_ty = body_result?;

        let ret = match declared {
            Some(Type::Void) => Type::Void,
            Some(declared) => {
                let ends_with_expression = match body.unlocated() {
                    AstNode::Block(statements) => {
                        statements.last().is_some_and(|s| !is_statement(s))
                    }
                    _ => true,
                };
                if body_ty == Type::Never {
                    // Every path returns explicitly.
                } else if !ends_with_expression || body_ty == Type::Void {
                    return self.err(format!(
                        "'{}' must return a value of type {} (add a 'return' or end with an expression)",
                        name,
                        self.show(&declared)
                    ));
                } else if !self.assignable(&declared, &body_ty) {
                    return self.err(format!(
                        "'{}' returns {}, but its declared return type is {}",
                        name,
                        self.show(&body_ty),
                        self.show(&declared)
                    ));
                }
                declared
            }
            None => {
                let inferred = context.and_then(|c| c.inferred);
                let is_block = matches!(body.unlocated(), AstNode::Block(_));
                match (inferred, body_ty) {
                    (Some(inferred), Type::Never) => inferred,
                    (Some(inferred), tail) if is_block && !matches!(tail, Type::Void) => {
                        self.join(&inferred, &tail).unwrap_or(inferred)
                    }
                    (Some(inferred), _) => inferred,
                    (None, Type::Never) => Type::Void,
                    (None, tail) => tail,
                }
            }
        };
        Ok(Type::Function {
            params: param_types,
            ret: Box::new(ret),
        })
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    /// Checks an expression whose value is used: it must not be `void`.
    fn check_value(&mut self, node: &AstNode, expected: Option<&Type>) -> CheckResult<Type> {
        let ty = self.check_expr(node, expected)?;
        if ty == Type::Void {
            let hint = match node.unlocated() {
                AstNode::If { .. } | AstNode::Match { .. } => {
                    " (to be used as a value, every branch must produce a value of the same type, and 'if' needs an 'else')"
                }
                _ => "",
            };
            return self.err(format!("this expression has no value{}", hint));
        }
        Ok(ty)
    }

    fn check_expr(&mut self, node: &AstNode, expected: Option<&Type>) -> CheckResult<Type> {
        match node {
            AstNode::Located { line, node } => {
                self.state.line = *line;
                self.check_expr(node, expected)
            }
            AstNode::IntLiteral(_) => Ok(Type::Int),
            AstNode::FloatLiteral(_) => Ok(Type::Float),
            AstNode::StringLiteral(_) => Ok(Type::String),
            AstNode::BooleanLiteral(_) => Ok(Type::Boolean),
            AstNode::TemplateString(parts) => {
                for part in parts {
                    if let TemplatePart::Expression(expression) = part {
                        self.check_value(expression, None)?;
                    }
                }
                Ok(Type::String)
            }
            AstNode::ArrayLiteral(elements) => {
                let expected_element = match expected {
                    Some(Type::List(inner)) => Some((**inner).clone()),
                    _ => None,
                };
                let mut element_ty = Type::Unknown;
                for element in elements {
                    let ty = self.check_value(element, expected_element.as_ref())?;
                    element_ty = match self.join(&element_ty, &ty) {
                        Some(joined) => joined,
                        None => {
                            return self.err(format!(
                                "list elements must have the same type: found {} and {}",
                                self.show(&element_ty),
                                self.show(&ty)
                            ))
                        }
                    };
                }
                if let Some(expected_element) = expected_element {
                    if self.assignable(&expected_element, &element_ty) {
                        element_ty = expected_element;
                    }
                }
                Ok(Type::List(Box::new(element_ty)))
            }
            AstNode::ObjectLiteral(entries) => self.check_object(entries, expected),
            AstNode::Identifier(name) => self.check_identifier(name, expected),
            AstNode::Unary { op, operand } => {
                let ty = self.check_value(operand, None)?;
                match (op, &ty) {
                    (_, Type::Any) => Ok(Type::Any),
                    (UnaryOperator::Negate, Type::Int | Type::Float) => Ok(ty),
                    (UnaryOperator::Not, Type::Boolean) => Ok(Type::Boolean),
                    (UnaryOperator::BitNot, Type::Int) => Ok(Type::Int),
                    (op, ty) => {
                        let symbol = match op {
                            UnaryOperator::Negate => "-",
                            UnaryOperator::Not => "not",
                            UnaryOperator::BitNot => "~",
                        };
                        self.err(format!("operator '{}' cannot be applied to {}", symbol, self.show(ty)))
                    }
                }
            }
            AstNode::BinaryOp { left, op, right } => {
                if *op == BinaryOperator::Is {
                    if let Some(result) = self.check_variant_test(left, right)? {
                        return Ok(result);
                    }
                }
                let left_ty = self.check_value(left, None)?;
                let right_ty = self.check_value(right, None)?;
                self.binary_type(*op, &left_ty, &right_ty)
            }
            AstNode::Call { callee, args } => self.check_call(callee, args, expected),
            AstNode::Index { target, index } => {
                let target_ty = self.check_value(target, None)?;
                let index_ty = self.check_value(index, None)?;
                match &target_ty {
                    Type::List(element) => {
                        self.expect_type(&Type::Int, &index_ty, "list index")?;
                        Ok((**element).clone())
                    }
                    Type::String => {
                        self.expect_type(&Type::Int, &index_ty, "string index")?;
                        Ok(Type::String)
                    }
                    Type::Any | Type::Unknown => Ok(Type::Any),
                    Type::Named { id, .. } => match &self.table.get(*id).kind {
                        TypeKind::Dict { key, value } => {
                            let (key, value) = (key.clone(), value.clone());
                            self.expect_type(&key, &index_ty, "dictionary key")?;
                            Ok(value)
                        }
                        _ => self.err(format!("cannot index a value of type {}", self.show(&target_ty))),
                    },
                    other => self.err(format!("cannot index a value of type {}", self.show(other))),
                }
            }
            AstNode::Member { object, name } => self.check_member(object, name),
            AstNode::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_condition(condition, "if")?;
                let narrowed = self.narrowing(condition);
                match else_branch {
                    None => {
                        self.check_block(then_branch, None, narrowed)?;
                        Ok(Type::Void)
                    }
                    Some(else_branch) => {
                        let then_ty = self.check_block(then_branch, expected, narrowed)?;
                        let else_ty = self.check_block(else_branch, expected, vec![])?;
                        self.branch_types(&[then_ty, else_ty], expected, "if")
                    }
                }
            }
            AstNode::Match { scrutinee, arms } => self.check_match(scrutinee, arms, expected),
            AstNode::Lambda {
                params,
                return_type,
                body,
            } => {
                let signature =
                    self.function_signature("anonymous function", params, return_type.as_ref(), false)?;
                self.check_function("anonymous function", params, signature, body)
            }
            AstNode::Block(_) => self.check_block(node, expected, vec![]),
            statement => self.check_statement(statement, expected),
        }
    }

    /// Type of an `if`/`match` with several branches.
    fn branch_types(&self, types: &[Type], expected: Option<&Type>, what: &str) -> CheckResult<Type> {
        if let Some(expected) = expected {
            for ty in types {
                if *ty != Type::Void && !self.assignable(expected, ty) {
                    return self.err(format!(
                        "{} branch has type {}, expected {}",
                        what,
                        self.show(ty),
                        self.show(expected)
                    ));
                }
            }
        }
        let mut result = Type::Never;
        for ty in types {
            result = match self.join(&result, ty) {
                Some(joined) => joined,
                // Branches with different types: fine as a statement, an error if used as a value.
                None => return Ok(Type::Void),
            };
        }
        Ok(result)
    }

    /// `x is Some` inside an `if` condition: in the `then` branch, `x` is known to be that variant.
    fn narrowing(&self, condition: &AstNode) -> Vec<(String, Symbol)> {
        if let AstNode::BinaryOp {
            left,
            op: BinaryOperator::Is,
            right,
        } = condition.unlocated()
        {
            if let (AstNode::Identifier(name), AstNode::Identifier(variant)) =
                (left.unlocated(), right.unlocated())
            {
                if let (Some(symbol), Some(Symbol {
                    kind: SymbolKind::Constructor { id: variant_type, .. },
                    ..
                })) = (self.lookup(name), self.lookup(variant))
                {
                    if let Type::Named { id, args } = &symbol.ty {
                        if id == variant_type {
                            let mut narrowed = symbol.clone();
                            narrowed.ty = Type::Variant {
                                id: *id,
                                args: args.clone(),
                                variant: variant.clone(),
                            };
                            return vec![(name.clone(), narrowed)];
                        }
                    }
                }
            }
        }
        vec![]
    }

    /// `value is Variant`: checks whether a sum-type value is a given variant.
    fn check_variant_test(&mut self, left: &AstNode, right: &AstNode) -> CheckResult<Option<Type>> {
        let variant = match right.unlocated() {
            AstNode::Identifier(name) => name,
            _ => return Ok(None),
        };
        let variant_type = match self.lookup(variant) {
            Some(Symbol {
                kind: SymbolKind::Constructor { id, .. },
                ..
            }) => *id,
            _ => return Ok(None),
        };
        let left_ty = self.check_value(left, None)?;
        match &left_ty {
            Type::Named { id, .. } | Type::Variant { id, .. } if *id == variant_type => {
                Ok(Some(Type::Boolean))
            }
            Type::Any | Type::Unknown => Ok(Some(Type::Boolean)),
            other => self.err(format!(
                "'{}' is not a variant of {}",
                variant,
                self.show(other)
            )),
        }
    }

    pub fn binary_type(&self, op: BinaryOperator, left: &Type, right: &Type) -> CheckResult<Type> {
        use BinaryOperator::*;
        let comparison = matches!(op, Equal | NotEqual | Greater | GreaterEqual | Less | LessEqual | Is | In);
        if left.is_dynamic() || right.is_dynamic() {
            return Ok(if comparison || matches!(op, And | Or) {
                Type::Boolean
            } else {
                Type::Any
            });
        }
        let numeric = left.is_numeric() && right.is_numeric();
        let both_int = *left == Type::Int && *right == Type::Int;
        let result = match op {
            Add if both_int => Some(Type::Int),
            Add if numeric => Some(Type::Float),
            Add if *left == Type::String && *right == Type::String => Some(Type::String),
            Add => match (left, right) {
                (Type::List(_), Type::List(_)) => self.join(left, right),
                _ => None,
            },
            // Like Go and Rust: dividing ints gives an int, dividing floats gives a float.
            Subtract | Multiply | Divide | Modulo | Power if both_int => Some(Type::Int),
            Subtract | Multiply | Divide | Modulo | Power if numeric => Some(Type::Float),
            Multiply => match (left, right) {
                (Type::String, Type::Int) | (Type::Int, Type::String) => Some(Type::String),
                _ => None,
            },
            Greater | GreaterEqual | Less | LessEqual
                if numeric || (*left == Type::String && *right == Type::String) =>
            {
                Some(Type::Boolean)
            }
            Equal | NotEqual | Is
                if numeric || self.assignable(left, right) || self.assignable(right, left) =>
            {
                Some(Type::Boolean)
            }
            And | Or if *left == Type::Boolean && *right == Type::Boolean => Some(Type::Boolean),
            In => match right {
                Type::String if *left == Type::String => Some(Type::Boolean),
                Type::List(element) if self.assignable(element, left) || self.assignable(left, element) => {
                    Some(Type::Boolean)
                }
                Type::Named { id, .. } => match &self.table.get(*id).kind {
                    TypeKind::Dict { key, .. } if self.assignable(key, left) => Some(Type::Boolean),
                    _ => None,
                },
                _ => None,
            },
            BitAnd | BitOr | BitXor | ShiftLeft | ShiftRight if both_int => Some(Type::Int),
            _ => None,
        };
        match result {
            Some(ty) => Ok(ty),
            None => self.err(format!(
                "operator '{}' cannot be applied to {} and {}",
                op.symbol(),
                self.show(left),
                self.show(right)
            )),
        }
    }

    fn check_identifier(&mut self, name: &str, expected: Option<&Type>) -> CheckResult<Type> {
        let symbol = match self.lookup(name) {
            Some(symbol) => symbol.clone(),
            None => return self.err(format!("undefined variable '{}'", name)),
        };
        match symbol.kind {
            SymbolKind::Builtin(builtin) => {
                self.err(format!("'{}' is a built-in function and must be called", builtin))
            }
            SymbolKind::Constructor { id, .. } => {
                // `None` takes its generic arguments from the expected type, when known.
                match (expected, &symbol.ty) {
                    (Some(Type::Named { id: expected_id, args }), Type::Named { .. })
                        if *expected_id == id =>
                    {
                        Ok(Type::Named {
                            id,
                            args: args.clone(),
                        })
                    }
                    _ => Ok(symbol.ty),
                }
            }
            SymbolKind::Value => Ok(symbol.ty),
        }
    }

    fn check_object(&mut self, entries: &[crate::parser::ast::ObjectEntry], expected: Option<&Type>) -> CheckResult<Type> {
        let target = expected.and_then(|ty| match ty {
            Type::Named { id, .. } => Some((ty.clone(), self.table.get(*id).kind.clone())),
            _ => None,
        });
        match target {
            Some((ty, TypeKind::Shape(fields))) | Some((ty, TypeKind::Interface(fields))) => {
                let type_name = self.show(&ty);
                for entry in entries {
                    match fields.iter().find(|(n, _)| *n == entry.key) {
                        Some((_, field_ty)) => {
                            let value_ty = self.check_value(&entry.value, Some(field_ty))?;
                            if !self.assignable(field_ty, &value_ty) {
                                return self.err(format!(
                                    "field '{}' of {} must be {}, found {}",
                                    entry.key,
                                    type_name,
                                    self.show(field_ty),
                                    self.show(&value_ty)
                                ));
                            }
                        }
                        None => {
                            return self.err(format!("{} has no field '{}'", type_name, entry.key))
                        }
                    }
                }
                let missing: Vec<&str> = fields
                    .iter()
                    .filter(|(n, _)| !entries.iter().any(|e| e.key == *n))
                    .map(|(n, _)| n.as_str())
                    .collect();
                if !missing.is_empty() {
                    return self.err(format!(
                        "missing field(s) {} in {}",
                        missing.join(", "),
                        type_name
                    ));
                }
                Ok(ty)
            }
            Some((ty, TypeKind::Dict { value, .. })) => {
                for entry in entries {
                    let value_ty = self.check_value(&entry.value, Some(&value))?;
                    if !self.assignable(&value, &value_ty) {
                        return self.err(format!(
                            "value of '{}' must be {}, found {}",
                            entry.key,
                            self.show(&value),
                            self.show(&value_ty)
                        ));
                    }
                }
                Ok(ty)
            }
            _ => {
                let mut fields = Vec::new();
                for entry in entries {
                    let ty = self.check_value(&entry.value, None)?;
                    fields.push((entry.key.clone(), ty));
                }
                Ok(Type::Record(fields))
            }
        }
    }

    fn check_member(&mut self, object: &AstNode, name: &str) -> CheckResult<Type> {
        let object_ty = self.check_value(object, None)?;
        match &object_ty {
            Type::Module(id) => match self.modules[*id].members.get(name).cloned() {
                Some(symbol) => match symbol.kind {
                    SymbolKind::Builtin(builtin) => {
                        self.err(format!("'{}' is a built-in function and must be called", builtin))
                    }
                    _ => Ok(symbol.ty),
                },
                None => self.err(format!("module has no member '{}'", name)),
            },
            Type::Any | Type::Unknown => Ok(Type::Any),
            Type::String | Type::List(_) => {
                self.err(format!("method '{}' must be called: '{}()'", name, name))
            }
            other => match self.table.fields_of(other) {
                Some(fields) => match fields.into_iter().find(|(n, _)| n == name) {
                    Some((_, ty)) => Ok(ty),
                    None => self.err(format!("{} has no field '{}'", self.show(other), name)),
                },
                None => {
                    let hint = match other {
                        Type::Named { id, .. } if self.table.variants_of(*id).is_some() => {
                            " (use 'match' or check the variant with 'is' first)"
                        }
                        _ => "",
                    };
                    self.err(format!(
                        "a value of type {} has no field '{}'{}",
                        self.show(other),
                        name,
                        hint
                    ))
                }
            },
        }
    }

    // ------------------------------------------------------------------
    // Calls
    // ------------------------------------------------------------------

    fn check_call(&mut self, callee: &AstNode, args: &[AstNode], expected: Option<&Type>) -> CheckResult<Type> {
        match callee.unlocated() {
            AstNode::Identifier(name) => {
                if let Some(symbol) = self.lookup(name).cloned() {
                    match symbol.kind {
                        SymbolKind::Builtin(builtin) => return self.check_builtin_call(&builtin, args),
                        SymbolKind::Constructor { id, variant } => {
                            return self.check_constructor(id, &variant, args, expected)
                        }
                        SymbolKind::Value => {}
                    }
                }
            }
            AstNode::Member { object, name } => {
                let object_ty = self.check_value(object, None)?;
                return self.check_method_call(&object_ty, name, args, expected);
            }
            _ => {}
        }
        let callee_ty = self.check_value(callee, None)?;
        self.check_function_call(&callee_ty, args)
    }

    fn check_function_call(&mut self, callee_ty: &Type, args: &[AstNode]) -> CheckResult<Type> {
        match callee_ty {
            Type::Function { params, ret } => {
                self.check_args(params, args)?;
                Ok((**ret).clone())
            }
            Type::Any | Type::Unknown => {
                for arg in args {
                    self.check_value(arg, None)?;
                }
                Ok(Type::Any)
            }
            other => self.err(format!("a value of type {} cannot be called", self.show(other))),
        }
    }

    fn check_args(&mut self, params: &[Type], args: &[AstNode]) -> CheckResult<Vec<Type>> {
        if params.len() != args.len() {
            return self.err(format!(
                "expected {} argument(s), found {}",
                params.len(),
                args.len()
            ));
        }
        let mut types = Vec::new();
        for (i, (param, arg)) in params.iter().zip(args).enumerate() {
            let ty = self.check_value(arg, Some(param))?;
            if !self.assignable(param, &ty) {
                let hint = match (param, &ty) {
                    (Type::Named { .. }, Type::Named { .. })
                        if self.table.fields_of(param).is_some() && self.table.fields_of(&ty).is_some() =>
                    {
                        format!(
                            " ({} is not a subtype of {}: ez has no subtyping, pass the fields explicitly)",
                            self.show(&ty),
                            self.show(param)
                        )
                    }
                    _ => String::new(),
                };
                return self.err(format!(
                    "argument {} must be {}, found {}{}",
                    i + 1,
                    self.show(param),
                    self.show(&ty),
                    hint
                ));
            }
            types.push(ty);
        }
        Ok(types)
    }

    fn check_constructor(
        &mut self,
        id: TypeId,
        variant: &str,
        args: &[AstNode],
        expected: Option<&Type>,
    ) -> CheckResult<Type> {
        let info = self.table.get(id).clone();
        let fields = match info.kind {
            TypeKind::Sum(variants) => variants
                .into_iter()
                .find(|v| v.name == variant)
                .map(|v| v.fields)
                .unwrap_or_default(),
            _ => vec![],
        };
        if fields.is_empty() {
            return self.err(format!("variant '{}' has no fields and is used without '()'", variant));
        }
        if fields.len() != args.len() {
            return self.err(format!(
                "'{}' expects {} argument(s), found {}",
                variant,
                fields.len(),
                args.len()
            ));
        }
        // Generic arguments: start from the expected type, then infer from the arguments.
        let mut generic: Vec<Type> = match expected {
            Some(Type::Named { id: expected_id, args }) if *expected_id == id => args.clone(),
            _ => vec![Type::Unknown; info.params],
        };
        for ((field, field_ty), arg) in fields.iter().zip(args) {
            let wanted = field_ty.substitute(&generic);
            let actual = self.check_value(arg, Some(&wanted))?;
            if let Type::Param(i) = field_ty {
                if generic[*i].contains_unknown() {
                    generic[*i] = self.join(&generic[*i], &actual).unwrap_or(actual.clone());
                    continue;
                }
            }
            if !self.assignable(&wanted, &actual) {
                return self.err(format!(
                    "field '{}' of '{}' must be {}, found {}",
                    field,
                    variant,
                    self.show(&wanted),
                    self.show(&actual)
                ));
            }
        }
        Ok(Type::Named { id, args: generic })
    }

    fn arg_types(&mut self, args: &[AstNode]) -> CheckResult<Vec<Type>> {
        args.iter().map(|a| self.check_value(a, None)).collect()
    }

    fn arity(&self, name: &str, args: &[Type], min: usize, max: usize) -> CheckResult<()> {
        if args.len() < min || args.len() > max {
            let expected = if min == max {
                min.to_string()
            } else {
                format!("{} to {}", min, max)
            };
            return self.err(format!(
                "'{}' expects {} argument(s), found {}",
                name,
                expected,
                args.len()
            ));
        }
        Ok(())
    }

    fn expect_arg(&self, name: &str, index: usize, expected: &Type, actual: &Type) -> CheckResult<()> {
        if self.assignable(expected, actual) {
            Ok(())
        } else {
            self.err(format!(
                "argument {} of '{}' must be {}, found {}",
                index + 1,
                name,
                self.show(expected),
                self.show(actual)
            ))
        }
    }

    fn expect_numeric_arg(&self, name: &str, index: usize, actual: &Type) -> CheckResult<()> {
        if actual.is_numeric() || actual.is_dynamic() {
            Ok(())
        } else {
            self.err(format!(
                "argument {} of '{}' must be a number, found {}",
                index + 1,
                name,
                self.show(actual)
            ))
        }
    }

    /// Checks a call to a built-in function and returns its result type.
    fn check_builtin_call(&mut self, name: &str, args: &[AstNode]) -> CheckResult<Type> {
        let types = self.arg_types(args)?;
        let b = self.builtins;
        let s = Type::String;
        let check = |checker: &Self, expected: &[Type]| -> CheckResult<()> {
            checker.arity(name, &types, expected.len(), expected.len())?;
            for (i, (e, a)) in expected.iter().zip(&types).enumerate() {
                checker.expect_arg(name, i, e, a)?;
            }
            Ok(())
        };
        match name {
            "print" => Ok(Type::Void),
            "input" => {
                self.arity(name, &types, 0, 1)?;
                if let Some(prompt) = types.first() {
                    self.expect_arg(name, 0, &s, prompt)?;
                }
                Ok(self.option_of(Type::String))
            }
            "range" => {
                self.arity(name, &types, 1, 3)?;
                for (i, ty) in types.iter().enumerate() {
                    self.expect_arg(name, i, &Type::Int, ty)?;
                }
                Ok(Type::List(Box::new(Type::Int)))
            }
            "int" | "float" => {
                self.arity(name, &types, 1, 1)?;
                let target = if name == "int" { Type::Int } else { Type::Float };
                match &types[0] {
                    Type::Int | Type::Float | Type::Boolean => Ok(target),
                    Type::String => Ok(self.result_of(target, Type::String)),
                    Type::Any | Type::Unknown => Ok(Type::Any),
                    other => self.err(format!("cannot convert {} to {}", self.show(other), name)),
                }
            }
            "string" => {
                self.arity(name, &types, 1, 1)?;
                Ok(Type::String)
            }

            "time.sleep" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(Type::Void)
            }
            "time.now" | "time.monotonic" => {
                check(self, &[])?;
                Ok(Type::Float)
            }

            "file.open" => {
                check(self, &[s.clone(), s])?;
                Ok(Self::named(b.file))
            }

            "datetime.now" => {
                check(self, &[])?;
                Ok(Self::named(b.datetime))
            }
            "datetime.today" => {
                check(self, &[])?;
                Ok(Type::String)
            }
            "datetime.format" => {
                self.arity(name, &types, 2, 2)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                self.expect_arg(name, 1, &s, &types[1])?;
                Ok(Type::String)
            }
            "datetime.parse" => {
                check(self, &[s.clone(), s])?;
                Ok(Type::Int)
            }

            "math.abs" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(types[0].clone())
            }
            "math.sqrt" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(Type::Float)
            }
            "math.pow" => {
                self.arity(name, &types, 2, 2)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                self.expect_numeric_arg(name, 1, &types[1])?;
                Ok(Type::Float)
            }
            "math.floor" | "math.ceil" | "math.round" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(Type::Int)
            }

            "http.get" | "http.delete" => {
                check(self, &[s])?;
                Ok(Self::named(b.response))
            }
            "http.post" | "http.put" => {
                check(self, &[s.clone(), s])?;
                Ok(Self::named(b.response))
            }

            "json.parse" => {
                check(self, &[s])?;
                Ok(Type::Any)
            }
            "json.stringify" => {
                self.arity(name, &types, 1, 1)?;
                Ok(Type::String)
            }

            "regex.match" => {
                check(self, &[s.clone(), s])?;
                Ok(Type::Boolean)
            }
            "regex.search" => {
                check(self, &[s.clone(), s])?;
                Ok(self.option_of(Self::named(b.regex_match)))
            }
            "regex.findAll" => {
                check(self, &[s.clone(), s])?;
                Ok(Type::List(Box::new(Self::named(b.regex_match))))
            }
            "regex.replace" => {
                check(self, &[s.clone(), s.clone(), s])?;
                Ok(Type::String)
            }
            other => self.err(format!("unknown built-in function '{}'", other)),
        }
    }

    fn check_method_call(
        &mut self,
        object_ty: &Type,
        name: &str,
        args: &[AstNode],
        expected: Option<&Type>,
    ) -> CheckResult<Type> {
        if let Type::Module(id) = object_ty {
            let symbol = match self.modules[*id].members.get(name) {
                Some(symbol) => symbol.clone(),
                None => return self.err(format!("module has no member '{}'", name)),
            };
            return match symbol.kind {
                SymbolKind::Builtin(builtin) => self.check_builtin_call(&builtin, args),
                SymbolKind::Constructor { id, variant } => {
                    self.check_constructor(id, &variant, args, expected)
                }
                SymbolKind::Value => self.check_function_call(&symbol.ty, args),
            };
        }

        let s = Type::String;
        let signature: Option<(Vec<Type>, Type)> = match object_ty {
            Type::String => match name {
                "length" => Some((vec![], Type::Int)),
                "trim" | "upper" | "lower" => Some((vec![], Type::String)),
                "isSpace" | "isNumeric" | "isAlpha" | "isAlphaNumeric" | "isUpper" | "isLower"
                | "isTitle" => Some((vec![], Type::Boolean)),
                "split" => Some((vec![s.clone()], Type::List(Box::new(Type::String)))),
                "replace" => Some((vec![s.clone(), s], Type::String)),
                _ => None,
            },
            Type::List(_) => match name {
                "length" => Some((vec![], Type::Int)),
                _ => None,
            },
            Type::Any | Type::Unknown => {
                for arg in args {
                    self.check_value(arg, None)?;
                }
                return Ok(Type::Any);
            }
            Type::Named { id, .. } => match &self.table.get(*id).kind {
                TypeKind::Dict { .. } if name == "length" => Some((vec![], Type::Int)),
                TypeKind::Builtin { methods, .. } => methods
                    .iter()
                    .find(|(n, _, _)| n == name)
                    .map(|(_, params, ret)| (params.clone(), ret.clone())),
                _ => None,
            },
            _ => None,
        };
        if let Some((params, ret)) = signature {
            self.check_args(&params, args)?;
            return Ok(ret);
        }

        // A field holding a function: `obj.callback(1)`.
        if let Some(fields) = self.table.fields_of(object_ty) {
            if let Some((_, field_ty)) = fields.into_iter().find(|(n, _)| n == name) {
                return self.check_function_call(&field_ty, args);
            }
        }
        self.err(format!(
            "a value of type {} has no method '{}'",
            self.show(object_ty),
            name
        ))
    }

    // ------------------------------------------------------------------
    // Match
    // ------------------------------------------------------------------

    fn check_match(&mut self, scrutinee: &AstNode, arms: &[MatchArm], expected: Option<&Type>) -> CheckResult<Type> {
        let scrutinee_ty = self.check_value(scrutinee, None)?;
        let mut arm_types = Vec::new();
        for arm in arms {
            self.push_scope();
            let result = (|| -> CheckResult<Type> {
                self.check_pattern(&arm.pattern, &scrutinee_ty)?;
                if let Some(guard) = &arm.guard {
                    self.check_condition(guard, "match guard")?;
                }
                self.check_block(&arm.body, expected, vec![])
            })();
            self.pop_scope();
            arm_types.push(result?);
        }
        self.check_exhaustive(&scrutinee_ty, arms)?;
        self.branch_types(&arm_types, expected, "match arm")
    }

    /// Whether `name` in a pattern refers to a variant of `ty`.
    fn variant_of(&self, ty: &Type, name: &str) -> Option<(TypeId, Vec<Type>, VariantDef)> {
        let (id, args) = match ty {
            Type::Named { id, args } | Type::Variant { id, args, .. } => (*id, args.clone()),
            _ => return None,
        };
        self.table
            .variants_of(id)?
            .iter()
            .find(|v| v.name == name)
            .map(|v| (id, args, v.clone()))
    }

    fn is_constructor_name(&self, name: &str) -> bool {
        matches!(
            self.lookup(name),
            Some(Symbol {
                kind: SymbolKind::Constructor { .. },
                ..
            })
        )
    }

    fn check_pattern(&mut self, pattern: &Pattern, ty: &Type) -> CheckResult<()> {
        match pattern {
            Pattern::Wildcard => Ok(()),
            Pattern::Identifier(name) => {
                if let Some((_, _, variant)) = self.variant_of(ty, name) {
                    if !variant.fields.is_empty() {
                        return self.err(format!(
                            "variant '{}' has fields; use '{}(...)' in the pattern",
                            name, name
                        ));
                    }
                    return Ok(());
                }
                if self.is_constructor_name(name) && !ty.is_dynamic() {
                    return self.err(format!(
                        "pattern '{}' does not match a value of type {}",
                        name,
                        self.show(ty)
                    ));
                }
                if ty.is_dynamic() && self.is_constructor_name(name) {
                    return Ok(());
                }
                self.current_scope()
                    .vars
                    .insert(name.clone(), Symbol::value(ty.clone(), false));
                Ok(())
            }
            Pattern::Variant { name, args } => {
                if ty.is_dynamic() {
                    for arg in args {
                        self.check_pattern(arg, &Type::Any)?;
                    }
                    return Ok(());
                }
                let (_, type_args, variant) = match self.variant_of(ty, name) {
                    Some(found) => found,
                    None => {
                        return self.err(format!(
                            "'{}' is not a variant of {}",
                            name,
                            self.show(ty)
                        ))
                    }
                };
                if variant.fields.len() != args.len() {
                    return self.err(format!(
                        "variant '{}' has {} field(s), but the pattern has {}",
                        name,
                        variant.fields.len(),
                        args.len()
                    ));
                }
                for ((_, field_ty), arg) in variant.fields.iter().zip(args) {
                    self.check_pattern(arg, &field_ty.substitute(&type_args))?;
                }
                Ok(())
            }
            Pattern::Literal(literal) => {
                let literal_ty = self.check_expr(literal, None)?;
                if !self.assignable(ty, &literal_ty) && !self.assignable(&literal_ty, ty) {
                    return self.err(format!(
                        "pattern of type {} does not match a value of type {}",
                        self.show(&literal_ty),
                        self.show(ty)
                    ));
                }
                Ok(())
            }
        }
    }

    /// Pattern matches every value of its type.
    fn irrefutable(&self, pattern: &Pattern, ty: &Type) -> bool {
        match pattern {
            Pattern::Wildcard => true,
            Pattern::Identifier(name) => self.variant_of(ty, name).is_none() && !self.is_constructor_name(name),
            _ => false,
        }
    }

    fn check_exhaustive(&self, ty: &Type, arms: &[MatchArm]) -> CheckResult<()> {
        let rows: Vec<Vec<&Pattern>> = arms
            .iter()
            .filter(|a| a.guard.is_none())
            .map(|a| vec![&a.pattern])
            .collect();
        match self.missing_pattern(std::slice::from_ref(ty), rows) {
            None => Ok(()),
            Some(witness) => {
                let witness = witness.into_iter().next().unwrap_or_else(|| "_".to_string());
                if witness == "_" {
                    self.err("match is not exhaustive: add a wildcard arm '_ => ...'".to_string())
                } else {
                    self.err(format!(
                        "match is not exhaustive: {} is not handled (add an arm for it or a wildcard '_ => ...')",
                        witness
                    ))
                }
            }
        }
    }

    /// Constructors of a type with finitely many of them: name and field types.
    fn constructors(&self, ty: &Type) -> Option<Vec<(String, Vec<Type>)>> {
        match ty {
            Type::Named { id, args } | Type::Variant { id, args, .. } => {
                let variants = self.table.variants_of(*id)?;
                let only = match ty {
                    Type::Variant { variant, .. } => Some(variant),
                    _ => None,
                };
                Some(
                    variants
                        .iter()
                        .filter(|v| only.is_none_or(|o| *o == v.name))
                        .map(|v| {
                            (
                                v.name.clone(),
                                v.fields.iter().map(|(_, t)| t.substitute(args)).collect(),
                            )
                        })
                        .collect(),
                )
            }
            Type::Boolean => Some(vec![
                ("true".to_string(), vec![]),
                ("false".to_string(), vec![]),
            ]),
            _ => None,
        }
    }

    /// Classic usefulness check on a pattern matrix: returns an example of values (one per
    /// column) that no row matches, or `None` when the rows are exhaustive.
    fn missing_pattern(&self, types: &[Type], rows: Vec<Vec<&Pattern>>) -> Option<Vec<String>> {
        let (first, rest) = match types.split_first() {
            None => return if rows.is_empty() { Some(vec![]) } else { None },
            Some(split) => split,
        };
        if first.is_dynamic() {
            return None;
        }
        let wildcard = Pattern::Wildcard;

        let head_name = |pattern: &Pattern| -> Option<String> {
            match pattern {
                Pattern::Identifier(n) | Pattern::Variant { name: n, .. } => Some(n.clone()),
                Pattern::Literal(AstNode::BooleanLiteral(b)) => Some(b.to_string()),
                _ => None,
            }
        };
        let constructors = self.constructors(first).filter(|constructors| {
            // Split by constructor only when the column mentions all of them; otherwise the
            // rows with an irrefutable head (the default matrix) must cover the rest.
            !constructors.is_empty()
                && constructors.iter().all(|(name, _)| {
                    rows.iter().any(|row| {
                        !self.irrefutable(row[0], first) && head_name(row[0]).as_deref() == Some(name)
                    })
                })
        });
        if let Some(constructors) = constructors {
            for (name, fields) in constructors {
                let mut specialized: Vec<Vec<&Pattern>> = Vec::new();
                for row in &rows {
                    let head = row[0];
                    let args: Option<Vec<&Pattern>> = match head {
                        _ if self.irrefutable(head, first) => Some(vec![&wildcard; fields.len()]),
                        Pattern::Identifier(n) if *n == name => Some(vec![]),
                        Pattern::Variant { name: n, args } if *n == name => Some(args.iter().collect()),
                        Pattern::Literal(AstNode::BooleanLiteral(b)) if b.to_string() == name => Some(vec![]),
                        _ => None,
                    };
                    if let Some(mut args) = args {
                        args.extend(row[1..].iter().copied());
                        specialized.push(args);
                    }
                }
                let mut sub_types = fields.clone();
                sub_types.extend(rest.iter().cloned());
                if let Some(witness) = self.missing_pattern(&sub_types, specialized) {
                    let (inner, outer) = witness.split_at(fields.len());
                    let head = if inner.is_empty() {
                        name
                    } else {
                        format!("{}({})", name, inner.join(", "))
                    };
                    let mut result = vec![head];
                    result.extend(outer.iter().cloned());
                    return Some(result);
                }
            }
            return None;
        }

        // Only rows with an irrefutable head cover the values not mentioned in the column.
        let defaults: Vec<Vec<&Pattern>> = rows
            .iter()
            .filter(|row| self.irrefutable(row[0], first))
            .map(|row| row[1..].to_vec())
            .collect();
        let missing_head = self.constructors(first).and_then(|constructors| {
            constructors
                .into_iter()
                .find(|(name, _)| {
                    !rows.iter().any(|row| {
                        !self.irrefutable(row[0], first) && head_name(row[0]).as_deref() == Some(name)
                    })
                })
                .map(|(name, fields)| {
                    if fields.is_empty() {
                        name
                    } else {
                        format!("{}({})", name, vec!["_"; fields.len()].join(", "))
                    }
                })
        });
        self.missing_pattern(rest, defaults).map(|witness| {
            let mut result = vec![missing_head.unwrap_or_else(|| "_".to_string())];
            result.extend(witness);
            result
        })
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

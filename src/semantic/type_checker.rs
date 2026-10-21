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
    /// A function parameter (read-only unless it is `mut`).
    is_param: bool,
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
            is_param: false,
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
            is_param: false,
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
    pub fn new() -> Self {
        let mut table = TypeTable::default();

        // Built-in sum types: declared first, resolved once every name exists (`Json` refers to
        // `JsonObject`, which refers back to `Json`).
        let mut ids: HashMap<&'static str, TypeId> = HashMap::new();
        for sum in BUILTIN_SUMS {
            ids.insert(sum.name, table.add(sum.name, sum.params, TypeKind::Pending));
        }
        let json = ids["Json"];
        let json_object = table.add(
            "JsonObject",
            0,
            TypeKind::Dict {
                key: Type::String,
                value: Self::named(json),
            },
        );
        let field_type = |name: &str| -> Type {
            match name {
                "T0" => Type::Param(0),
                "T1" => Type::Param(1),
                "string" => Type::String,
                "int" => Type::Int,
                "float" => Type::Float,
                "boolean" => Type::Boolean,
                "Json[]" => Type::List(Box::new(Self::named(json))),
                "JsonObject" => Self::named(json_object),
                other => panic!("unknown field type '{}' in a built-in sum type", other),
            }
        };
        for sum in BUILTIN_SUMS {
            let variants = sum
                .variants
                .iter()
                .map(|(name, fields)| VariantDef {
                    name: name.to_string(),
                    fields: fields
                        .iter()
                        .map(|(field, ty)| (field.to_string(), field_type(ty)))
                        .collect(),
                })
                .collect();
            table.types[ids[sum.name]].kind = TypeKind::Sum(variants);
        }

        let result_of = |ok: Type, error: &str| Type::Named {
            id: ids["Result"],
            args: vec![ok, Self::named(ids[error])],
        };
        let file = table.add(
            "File",
            0,
            TypeKind::Builtin {
                fields: vec![],
                methods: vec![
                    ("read".to_string(), vec![], result_of(Type::String, "FileError")),
                    (
                        "write".to_string(),
                        vec![Type::String],
                        result_of(Type::Void, "FileError"),
                    ),
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
                option: ids["Option"],
                result: ids["Result"],
                parse_error: ids["ParseError"],
                math_error: ids["MathError"],
                file_error: ids["FileError"],
                file_mode: ids["Mode"],
                http_error: ids["HttpError"],
                json,
                json_object,
                json_error: ids["JsonError"],
                date_error: ids["DateError"],
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
            methods: HashMap::new(),
            state: ModuleState::default(),
        }
    }

    /// Id of a built-in sum type, by name.
    fn builtin_sum_id(&self, name: &str) -> TypeId {
        let b = &self.builtins;
        match name {
            "Option" => b.option,
            "Result" => b.result,
            "ParseError" => b.parse_error,
            "MathError" => b.math_error,
            "FileError" => b.file_error,
            "Mode" => b.file_mode,
            "HttpError" => b.http_error,
            "Json" => b.json,
            "JsonError" => b.json_error,
            "DateError" => b.date_error,
            other => panic!("unknown built-in sum type '{}'", other),
        }
    }

    /// Scope with the built-in functions, types and constructors.
    fn prelude(&self) -> Scope {
        let mut scope = Scope::default();
        for name in ["print", "input", "range", "int", "float", "string", "panic"] {
            scope.vars.insert(name.to_string(), Symbol::builtin(name));
        }
        let primitives = [
            ("int", Type::Int),
            ("float", Type::Float),
            ("string", Type::String),
            ("boolean", Type::Boolean),
            ("bool", Type::Boolean),
            ("void", Type::Void),
        ];
        for (name, ty) in primitives {
            scope.types.insert(name.to_string(), TypeRef::Type(ty));
        }
        for sum in BUILTIN_SUMS.iter().filter(|sum| sum.module.is_none()) {
            let id = self.builtin_sum_id(sum.name);
            let type_ref = if sum.params > 0 {
                TypeRef::Generic(id)
            } else {
                TypeRef::Type(Self::named(id))
            };
            scope.types.insert(sum.name.to_string(), type_ref);
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
                        is_imut: true,
                        kind: SymbolKind::Constructor {
                            id,
                            variant: variant.name.clone(),
                        },
                        owner: None,
                        mut_params: Vec::new(),
                        is_param: false,
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
                is_entry: self.loading.len() == 1,
                scopes: vec![prelude, Scope::default()],
                function_scopes: Vec::new(),
                returns: Vec::new(),
                file: display.to_string(),
                path: path.to_path_buf(),
                dir,
                line: 0,
            },
        );

        let result = self.check_statements(&program, None).and_then(|_| {
            let module_scope = self.state.scopes.get(1).cloned().unwrap_or_default();
            self.module_exports(&program, &module_scope, display)
        });
        self.state = saved;
        self.loading.pop();
        let module = result?;

        self.modules.push(module);
        let id = self.modules.len() - 1;
        self.module_by_path.insert(path.to_path_buf(), id);
        Ok(id)
    }

    /// What other modules can import: only the declarations marked with `export`. Exporting a
    /// sum type also exports its variants; an exported `var` is read-only outside the module.
    fn module_exports(&mut self, program: &[AstNode], scope: &Scope, display: &str) -> CheckResult<ModuleType> {
        let mut module = ModuleType {
            display: display.to_string(),
            ..ModuleType::default()
        };
        let mut exported_ids: HashSet<TypeId> = HashSet::new();
        let mut private_ids: HashSet<TypeId> = HashSet::new();
        for statement in program {
            let name = match statement.declared_name() {
                Some(name) => name,
                None => continue,
            };
            let is_type = matches!(
                statement.unlocated(),
                AstNode::TypeDecl { .. }
            );
            if !statement.is_exported() {
                module.private.insert(name.to_string());
                if let Some(TypeRef::Type(Type::Named { id, .. })) = scope.types.get(name).filter(|_| is_type) {
                    private_ids.insert(*id);
                }
                continue;
            }
            if is_type {
                if let Some(type_ref) = scope.types.get(name) {
                    module.types.insert(name.to_string(), type_ref.clone());
                    if let TypeRef::Type(Type::Named { id, .. }) = type_ref {
                        exported_ids.insert(*id);
                        for variant in self.table.variants_of(*id).cloned().unwrap_or_default() {
                            if let Some(symbol) = scope.vars.get(&variant.name) {
                                module.members.insert(variant.name.clone(), symbol.clone());
                            }
                        }
                    }
                }
            } else if let Some(symbol) = scope.vars.get(name) {
                let mut symbol = symbol.clone();
                symbol.owner = Some(display.to_string());
                module.members.insert(name.to_string(), symbol);
            }
        }
        private_ids.retain(|id| !exported_ids.contains(id));
        // Implicit declarations (`x = 1`) and imported names are never exported either.
        for name in scope.vars.keys().chain(scope.types.keys()) {
            if !module.members.contains_key(name) && !module.types.contains_key(name) {
                module.private.insert(name.clone());
            }
        }

        // A public signature cannot mention a private type: importers could not name it.
        for statement in program.iter().filter(|s| s.is_exported()) {
            self.set_line(statement);
            let name = statement.declared_name().unwrap_or_default();
            let mut types: Vec<Type> = Vec::new();
            match scope.types.get(name) {
                Some(TypeRef::Type(Type::Named { id, args })) if module.types.contains_key(name) => {
                    match &self.table.get(*id).kind {
                        TypeKind::Sum(variants) => types.extend(
                            variants.iter().flat_map(|v| v.fields.iter().map(|(_, t)| t.clone())),
                        ),
                        TypeKind::Shape(fields) => {
                            types.extend(fields.iter().map(|(_, t)| t.clone()))
                        }
                        TypeKind::Dict { value, .. } => types.push(value.clone()),
                        _ => {}
                    }
                    types.extend(args.iter().cloned());
                }
                Some(TypeRef::Type(alias)) if module.types.contains_key(name) => types.push(alias.clone()),
                _ => {}
            }
            if let Some(symbol) = module.members.get(name) {
                types.push(symbol.ty.clone());
            }
            for ty in &types {
                if let Some(id) = Self::find_type_id(ty, &private_ids) {
                    return self.err(format!(
                        "'{}' is exported, but its type uses the private type '{}' (export '{}' too)",
                        name,
                        self.table.get(id).name,
                        self.table.get(id).name
                    ));
                }
            }
        }
        Ok(module)
    }

    fn missing_member<T>(&self, module: ModuleId, name: &str) -> CheckResult<T> {
        let module = &self.modules[module];
        if module.private.contains(name) {
            self.err(format!("'{}' is not exported by '{}'", name, module.display))
        } else {
            self.err(format!("module '{}' has no member '{}'", module.display, name))
        }
    }

    /// First type of `ids` that appears in `ty`.
    fn find_type_id(ty: &Type, ids: &HashSet<TypeId>) -> Option<TypeId> {
        match ty {
            Type::Named { id, args } | Type::Variant { id, args, .. } => {
                if ids.contains(id) {
                    return Some(*id);
                }
                args.iter().find_map(|a| Self::find_type_id(a, ids))
            }
            Type::List(inner) => Self::find_type_id(inner, ids),
            Type::Function { params, ret } => params
                .iter()
                .chain(std::iter::once(&**ret))
                .find_map(|t| Self::find_type_id(t, ids)),
            Type::Record(fields) => fields.iter().find_map(|(_, t)| Self::find_type_id(t, ids)),
            _ => None,
        }
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

    // ------------------------------------------------------------------
    // Type compatibility
    // ------------------------------------------------------------------

    /// Whether a value of type `actual` can be used where `expected` is required.
    pub fn assignable(&self, expected: &Type, actual: &Type) -> bool {
        match (expected, actual) {
            (_, Type::Never | Type::Unknown) | (Type::Unknown, _) => true,
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
                // Shapes are nominal: only an object literal with the same fields (or a value of
                // the same type, handled above) is accepted.
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
        // Imports first: types and function signatures may refer to imported modules.
        for statement in statements {
            self.set_line(statement);
            if let AstNode::Import { kind, path } = statement.unlocated() {
                self.check_import(kind, path)?;
            }
        }

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
                AstNode::TypeDecl { name, .. } => {
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
                _ => {}
            }
        }

        for statement in statements {
            self.set_line(statement);
            if let AstNode::FunctionDecl {
                name,
                owner,
                params,
                return_type,
                ..
            } = statement.unlocated()
            {
                if let Some(owner) = owner {
                    self.declare_method(statements, statement, owner, name, params, return_type.as_ref())?;
                    continue;
                }
                let ty = self.function_signature(name, params, return_type.as_ref(), true)?;
                let mut symbol = Symbol::value(ty, true);
                if params.iter().any(|p| p.is_mut) {
                    symbol.mut_params = params.iter().map(|p| p.is_mut).collect();
                }
                self.declare(name, symbol)?;
            }
        }
        Ok(())
    }

    /// `function Person.greet(self)` or `function Person.create(name: string)`: only at the top
    /// level of the file that declares `Person`.
    fn declare_method(
        &mut self,
        statements: &[AstNode],
        statement: &AstNode,
        owner: &str,
        name: &str,
        params: &[Param],
        return_type: Option<&TypeExpr>,
    ) -> CheckResult<()> {
        let full = format!("{}.{}", owner, name);
        if self.state.scopes.len() != 2 || !self.state.function_scopes.is_empty() {
            return self.err(format!(
                "'{}' must be declared at the top level of the file that declares '{}'",
                full, owner
            ));
        }
        if statement.is_exported() {
            return self.err(format!(
                "'{}' does not take 'export': methods are available wherever their type is",
                full
            ));
        }
        let declared_here = statements.iter().any(|s| {
            matches!(
                s.unlocated(),
                AstNode::TypeDecl { name, definition } if name == owner && !matches!(definition, TypeDef::Alias(_))
            )
        });
        let id = match self.current_scope().types.get(owner) {
            Some(TypeRef::Type(Type::Named { id, .. })) if declared_here => *id,
            _ => {
                return self.err(format!(
                    "cannot declare '{}': methods belong to a type with fields or variants declared in the same file",
                    full
                ))
            }
        };
        if self.methods.contains_key(&(id, name.to_string())) {
            return self.err(format!("'{}' is already declared", full));
        }
        let is_field = self
            .table
            .fields_of(&Self::named(id))
            .is_some_and(|fields| fields.iter().any(|(n, _)| n == name));
        let is_dict_length = name == "length" && matches!(self.table.get(id).kind, TypeKind::Dict { .. });
        if is_field || is_dict_length {
            return self.err(format!(
                "'{}' has the same name as a {} of '{}'",
                full,
                if is_field { "field" } else { "built-in method" },
                owner
            ));
        }

        let has_self = params.first().is_some_and(|p| p.name == "self");
        for (i, param) in params.iter().enumerate() {
            if param.name != "self" {
                continue;
            }
            if i > 0 {
                return self.err(format!("in '{}', 'self' must be the first parameter", full));
            }
            if param.is_mut {
                return self.err(format!(
                    "'mut self' is not available yet: '{}' cannot modify the value it is called on",
                    full
                ));
            }
            if param.ty.is_some() {
                return self.err(format!(
                    "'self' has no type annotation: in '{}' it is always '{}'",
                    full, owner
                ));
            }
        }
        let rest = if has_self { &params[1..] } else { params };
        let ty = match self.function_signature(&full, rest, return_type, true)? {
            Type::Function { mut params, ret } => {
                if has_self {
                    params.insert(0, Self::named(id));
                }
                Type::Function { params, ret }
            }
            other => other,
        };
        let info = MethodInfo {
            ty,
            has_self,
            mut_params: params.iter().map(|p| p.is_mut).collect(),
            module: self.state.path.clone(),
        };
        self.methods.insert((id, name.to_string()), info);
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
                        TypeKind::Shape(fields) => fields,
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
            AstNode::Export(node) => self.check_statement(node, expected),
            AstNode::VarDecl {
                name,
                ty,
                value,
                is_const,
                is_imut,
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
                let mut symbol = Symbol::value(var_ty, *is_const);
                symbol.is_imut = *is_imut;
                self.declare(name, symbol)?;
                Ok(Type::Void)
            }
            AstNode::Assign { target, op, value } => {
                self.check_assign(target, op, value)?;
                Ok(Type::Void)
            }
            AstNode::FunctionDecl {
                name,
                owner: Some(owner),
                params,
                body,
                ..
            } => {
                let full = format!("{}.{}", owner, name);
                let signature = match self.lookup_type(owner) {
                    Some(TypeRef::Type(Type::Named { id, .. })) => {
                        self.methods.get(&(*id, name.clone())).map(|m| m.ty.clone())
                    }
                    _ => None,
                };
                match signature {
                    Some(signature) => self.check_function(&full, params, signature, body)?,
                    None => return self.err(format!("unknown method '{}'", full)),
                };
                Ok(Type::Void)
            }
            AstNode::FunctionDecl {
                name,
                params,
                return_type,
                body,
                ..
            } => {
                let signature = match self.state.scopes.last().and_then(|s| s.vars.get(name)) {
                    Some(symbol) => symbol.ty.clone(),
                    None => self.function_signature(name, params, return_type.as_ref(), true)?,
                };
                self.check_function(name, params, signature, body)?;
                Ok(Type::Void)
            }
            AstNode::TypeDecl { .. } => Ok(Type::Void),
            // Imports are processed when the block is hoisted.
            AstNode::Import { .. } => Ok(Type::Void),
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
                    Type::Unknown => Type::Unknown,
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
        if !matches!(ty, Type::Boolean | Type::Unknown | Type::Never) {
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
                if let Some(owner) = &symbol.owner {
                    return self.err(format!("'{}' is read-only outside '{}'", name, owner));
                }
                self.ensure_not_captured(name)?;
                if !matches!(symbol.kind, SymbolKind::Value) || matches!(symbol.ty, Type::Module(_)) {
                    return self.err(format!("cannot assign to '{}'", name));
                }
                if symbol.is_imut {
                    return self.err(format!("cannot modify '{}': {}", name, self.imut_reason(&symbol)));
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
                self.ensure_contents_mutable(inner)?;
                let container = self.check_value(inner, None)?;
                let index_ty = self.check_value(index, None)?;
                let element = match &container {
                    Type::List(element) => {
                        self.expect_type(&Type::Int, &index_ty, "list index")?;
                        (**element).clone()
                    }
                    Type::Unknown => Type::Unknown,
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
                self.ensure_contents_mutable(object)?;
                let object_ty = self.check_value(object, None)?;
                let field_ty = match &object_ty {
                    Type::Unknown => Type::Unknown,
                    Type::Module(id) => {
                        return self.err(format!(
                            "'{}' is read-only outside '{}'",
                            name, self.modules[*id].display
                        ))
                    }
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

    /// Changing a field or an element (`person.age = 31`, `items[0] = 1`) is not allowed when the
    /// variable that holds the value is `imut`.
    fn ensure_contents_mutable(&self, target: &AstNode) -> CheckResult<()> {
        let mut node = target.unlocated();
        loop {
            match node {
                AstNode::Member { object, .. } => node = object.unlocated(),
                AstNode::Index { target, .. } => node = target.unlocated(),
                AstNode::Identifier(name) => {
                    return match self.lookup(name) {
                        Some(symbol) if symbol.is_imut && matches!(symbol.kind, SymbolKind::Value) => {
                            self.err(format!("cannot modify '{}': {}", name, self.imut_reason(symbol)))
                        }
                        _ => self.ensure_not_captured(name),
                    };
                }
                _ => return Ok(()),
            }
        }
    }

    fn imut_reason(&self, symbol: &Symbol) -> &'static str {
        if symbol.is_param {
            "parameters can only be modified when declared 'mut'"
        } else {
            "it is imut"
        }
    }

    /// What can be passed as `mut`: a `var` (not `const` or `imut`), or a field or an element
    /// of a value that can be modified. Returns its type.
    fn check_mut_place(&mut self, place: &AstNode) -> CheckResult<Type> {
        match place.unlocated() {
            AstNode::Identifier(name) => {
                let symbol = match self.lookup(name) {
                    Some(symbol) => symbol.clone(),
                    None => return self.err(format!("undefined variable '{}'", name)),
                };
                if let Some(owner) = &symbol.owner {
                    return self.err(format!("'{}' is read-only outside '{}'", name, owner));
                }
                self.ensure_not_captured(name)?;
                if !matches!(symbol.kind, SymbolKind::Value) || matches!(symbol.ty, Type::Module(_)) {
                    return self.err(format!("'{}' cannot be passed as 'mut'", name));
                }
                if symbol.is_imut {
                    return self.err(format!("cannot pass '{}' as 'mut': {}", name, self.imut_reason(&symbol)));
                }
                if symbol.is_const {
                    return self.err(format!("cannot pass '{}' as 'mut': it is const", name));
                }
                Ok(symbol.ty)
            }
            AstNode::Member { object, name } => {
                self.ensure_contents_mutable(object)?;
                let object_ty = self.check_value(object, None)?;
                let read_only = match &object_ty {
                    Type::Module(_) | Type::Variant { .. } => true,
                    Type::Named { id, .. } => matches!(self.table.get(*id).kind, TypeKind::Builtin { .. }),
                    _ => false,
                };
                if read_only {
                    return self.err(format!("'{}' cannot be passed as 'mut': it is read-only", name));
                }
                self.check_member(object, name)
            }
            AstNode::Index { target, .. } => {
                self.ensure_contents_mutable(target)?;
                if self.check_value(target, None)? == Type::String {
                    return self.err("strings cannot be modified".to_string());
                }
                self.check_value(place, None)
            }
            _ => self.err("only a variable, a field or an element can be passed as 'mut'".to_string()),
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
                    let display = path.to_string();
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
                        return self.missing_member(module_id, name);
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
        let mut module = ModuleType {
            display: name.to_string(),
            ..ModuleType::default()
        };
        for function in stdlib_functions(name) {
            module
                .members
                .insert(function.to_string(), Symbol::builtin(&format!("{}.{}", name, function)));
        }
        let b = self.builtins;
        if name == "math" {
            module.members.insert("pi".to_string(), Symbol::value(Type::Float, true));
            module.members.insert("e".to_string(), Symbol::value(Type::Float, true));
        }
        let object_types: &[(&str, TypeId)] = match name {
            "file" => &[("File", b.file)],
            "datetime" => &[("DateTime", b.datetime)],
            "http" => &[("Response", b.response)],
            "json" => &[("JsonObject", b.json_object)],
            "regex" => &[("Match", b.regex_match)],
            _ => &[],
        };
        for (type_name, id) in object_types {
            module
                .types
                .insert(type_name.to_string(), TypeRef::Type(Self::named(*id)));
        }
        // Sum types of the module (`FileError`, `Json`...), with their constructors.
        let mut constructors = Scope::default();
        for sum in BUILTIN_SUMS.iter().filter(|sum| sum.module == Some(name)) {
            let id = self.builtin_sum_id(sum.name);
            module
                .types
                .insert(sum.name.to_string(), TypeRef::Type(Self::named(id)));
            self.declare_constructors_into(&mut constructors, id);
        }
        module.members.extend(constructors.vars);
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
        self.state.function_scopes.push(self.state.scopes.len() - 1);
        for (param, ty) in params.iter().zip(&param_types) {
            if param.is_mut && name == "anonymous function" {
                return self.err(format!(
                    "anonymous functions cannot have 'mut' parameters ('{}')",
                    param.name
                ));
            }
            // Only `mut` parameters can be modified; the others (and `self`) are imut.
            let mut symbol = Symbol::value(ty.clone(), !param.is_mut);
            symbol.is_imut = !param.is_mut;
            symbol.is_param = true;
            self.declare(&param.name, symbol)?;
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
        self.state.function_scopes.pop();
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
                    (_, Type::Unknown) => Ok(Type::Unknown),
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
            AstNode::Call { callee, args, method } => self.check_call(callee, args, method, expected),
            AstNode::MutArg(_) => self.err("'mut' is only used before a call argument: 'f(mut x)'".to_string()),
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
                    Type::Unknown => Ok(Type::Unknown),
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
            AstNode::Try { value, wrap } => self.check_try(value, wrap),
            statement => self.check_statement(statement, expected),
        }
    }

    /// `result.mapErr(f)` with `f: (E) => F` gives a `Result[T, F]`.
    fn check_map_err(&mut self, type_args: &[Type], args: &[AstNode]) -> CheckResult<Type> {
        let ok = type_args.first().cloned().unwrap_or(Type::Unknown);
        let error = type_args.get(1).cloned().unwrap_or(Type::Unknown);
        if args.len() != 1 {
            return self.err(format!("'mapErr' expects 1 argument, found {}", args.len()));
        }
        let converter = self.check_value(&args[0], None)?;
        match converter {
            Type::Function { params, ret } if params.len() == 1 => {
                if !self.assignable(&params[0], &error) {
                    return self.err(format!(
                        "the function given to 'mapErr' must accept {}, but it accepts {}",
                        self.show(&error),
                        self.show(&params[0])
                    ));
                }
                Ok(Type::Named {
                    id: self.builtins.result,
                    args: vec![ok, *ret],
                })
            }
            other => self.err(format!(
                "'mapErr' expects a function with one parameter (or a variant with one field), found {}",
                self.show(&other)
            )),
        }
    }

    /// `value?` on a `Result` or an `Option`: its type is the `Ok` / `Some` value.
    fn check_try(&mut self, value: &AstNode, wrap: &RefCell<Option<TryWrap>>) -> CheckResult<Type> {
        let ty = self.check_value(value, None)?;
        let (id, args) = match &ty {
            Type::Named { id, args } if *id == self.builtins.result || *id == self.builtins.option => {
                (*id, args.clone())
            }
            Type::Unknown => return Ok(Type::Unknown),
            other => {
                return self.err(format!(
                    "'?' can only be used on a Result or an Option, found {}",
                    self.show(other)
                ))
            }
        };
        let inner = args.first().cloned().unwrap_or(Type::Unknown);
        let is_result = id == self.builtins.result;
        let kind = if is_result { "a Result" } else { "an Option" };

        let declared = match self.state.returns.last() {
            // At the top level of the main file, a failure stops the program.
            None if self.state.is_entry => return Ok(inner),
            None => {
                return self.err(
                    "'?' at the top level is only allowed in the main file".to_string(),
                )
            }
            Some(context) => context.declared.clone(),
        };
        let declared = match declared {
            Some(declared) => declared,
            None => {
                return self.err(
                    "'?' needs the function to declare its return type".to_string(),
                )
            }
        };
        let returned_args = match &declared {
            Type::Named { id: returned, args } if *returned == id => args.clone(),
            other => {
                return self.err(format!(
                    "'?' on {} needs the function to return {} too, but it returns {}",
                    kind,
                    kind,
                    self.show(other)
                ))
            }
        };
        if !is_result {
            return Ok(inner);
        }

        let error = args.get(1).cloned().unwrap_or(Type::Unknown);
        let target = returned_args.get(1).cloned().unwrap_or(Type::Unknown);
        if self.assignable(&target, &error) {
            return Ok(inner);
        }
        *wrap.borrow_mut() = Some(self.error_wrapper(&error, &target)?);
        Ok(inner)
    }

    /// Automatic conversion of `?`: the variant of `target` (the function's error type) that
    /// holds exactly one field of type `error`. It must be the only such variant.
    fn error_wrapper(&self, error: &Type, target: &Type) -> CheckResult<TryWrap> {
        let use_map_err = format!(
            "use '.mapErr(...)' to convert {} into {}",
            self.show(error),
            self.show(target)
        );
        let (target_id, target_args) = match target {
            Type::Named { id, args } if self.table.variants_of(*id).is_some() => (*id, args.clone()),
            _ => {
                return self.err(format!(
                    "'?' cannot convert {} into {}: {}",
                    self.show(error),
                    self.show(target),
                    use_map_err
                ))
            }
        };
        let candidates: Vec<String> = self
            .table
            .variants_of(target_id)
            .into_iter()
            .flatten()
            .filter(|variant| match variant.fields.as_slice() {
                [(_, field)] => {
                    let field = field.substitute(&target_args);
                    field == *error || (self.assignable(&field, error) && self.assignable(error, &field))
                }
                _ => false,
            })
            .map(|variant| variant.name.clone())
            .collect();
        let variant = match candidates.as_slice() {
            [variant] => variant.clone(),
            [] => {
                return self.err(format!(
                    "'?' cannot convert {} into {}: no variant of {} holds a {} ({})",
                    self.show(error),
                    self.show(target),
                    self.show(target),
                    self.show(error),
                    use_map_err
                ))
            }
            several => {
                return self.err(format!(
                    "'?' cannot choose how to convert {} into {}: the variants {} all hold a {} ({})",
                    self.show(error),
                    self.show(target),
                    several.join(", "),
                    self.show(error),
                    use_map_err
                ))
            }
        };

        // The runtime builds the variant by name: find how it is reachable from here.
        let is_target_constructor = |symbol: &Symbol| {
            matches!(&symbol.kind, SymbolKind::Constructor { id, .. } if *id == target_id)
        };
        if self.lookup(&variant).is_some_and(is_target_constructor) {
            return Ok(TryWrap {
                module: None,
                variant,
            });
        }
        for scope in self.state.scopes.iter().rev() {
            for (alias, symbol) in &scope.vars {
                if let Type::Module(module) = symbol.ty {
                    if self.modules[module]
                        .members
                        .get(&variant)
                        .is_some_and(is_target_constructor)
                    {
                        return Ok(TryWrap {
                            module: Some(alias.clone()),
                            variant,
                        });
                    }
                }
            }
        }
        self.err(format!(
            "'?' would wrap the error in '{}', but '{}' is not in scope here (import it)",
            variant, variant
        ))
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
            Type::Unknown => Ok(Some(Type::Boolean)),
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
                Type::Unknown
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
            SymbolKind::Value if !symbol.mut_params.is_empty() => self.err(format!(
                "'{}' has 'mut' parameters and can only be called, not used as a value",
                name
            )),
            SymbolKind::Value => Ok(symbol.ty),
        }
    }

    fn check_object(&mut self, entries: &[crate::parser::ast::ObjectEntry], expected: Option<&Type>) -> CheckResult<Type> {
        let target = expected.and_then(|ty| match ty {
            Type::Named { id, .. } => Some((ty.clone(), self.table.get(*id).kind.clone())),
            _ => None,
        });
        match target {
            Some((ty, TypeKind::Shape(fields))) => {
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
                    _ if !symbol.mut_params.is_empty() => self.err(format!(
                        "'{}' has 'mut' parameters and can only be called, not used as a value",
                        name
                    )),
                    _ => Ok(symbol.ty),
                },
                None => self.missing_member(*id, name),
            },
            Type::Unknown => Ok(Type::Unknown),
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

    fn check_call(
        &mut self,
        callee: &AstNode,
        args: &[AstNode],
        method: &RefCell<Option<MethodRef>>,
        expected: Option<&Type>,
    ) -> CheckResult<Type> {
        match callee.unlocated() {
            AstNode::Identifier(name) => {
                if let Some(symbol) = self.lookup(name).cloned() {
                    match symbol.kind {
                        SymbolKind::Builtin(builtin) => return self.check_builtin_call(&builtin, args),
                        SymbolKind::Constructor { id, variant } => {
                            return self.check_constructor(id, &variant, args, expected)
                        }
                        SymbolKind::Value if !symbol.mut_params.is_empty() => {
                            return self.check_function_call_mut(&symbol.ty, &symbol.mut_params, args)
                        }
                        SymbolKind::Value => {}
                    }
                }
            }
            AstNode::Member { object, name } => {
                // `Person.create(...)`: an associated function.
                if let Some(id) = self.type_path(object) {
                    return self.check_associated_call(id, name, args, method);
                }
                let object_ty = self.check_value(object, None)?;
                if let Type::Named { id, .. } = &object_ty {
                    if let Some(info) = self.methods.get(&(*id, name.clone())).cloned() {
                        return self.check_method(&object_ty, name, info, args, method);
                    }
                }
                return self.check_method_call(&object_ty, name, args, expected);
            }
            _ => {}
        }
        let callee_ty = self.check_value(callee, None)?;
        self.check_function_call(&callee_ty, args)
    }

    /// The type named by `Person` or `module.Person` when used before a `.`, if it is a type
    /// and not a value.
    fn type_path(&self, node: &AstNode) -> Option<TypeId> {
        let found = match node.unlocated() {
            AstNode::Identifier(name) if self.lookup(name).is_none() => self.lookup_type(name).cloned(),
            AstNode::Member { object, name } => match object.unlocated() {
                AstNode::Identifier(module) => match self.lookup(module).map(|s| &s.ty) {
                    Some(Type::Module(id)) if !self.modules[*id].members.contains_key(name) => {
                        self.modules[*id].types.get(name).cloned()
                    }
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        match found {
            Some(TypeRef::Type(Type::Named { id, .. })) => Some(id),
            _ => None,
        }
    }

    fn method_name(&self, id: TypeId, name: &str) -> String {
        format!("{}.{}", self.table.get(id).name, name)
    }

    /// `Person.create('Ana')`
    fn check_associated_call(
        &mut self,
        id: TypeId,
        name: &str,
        args: &[AstNode],
        method: &RefCell<Option<MethodRef>>,
    ) -> CheckResult<Type> {
        let full = self.method_name(id, name);
        let info = match self.methods.get(&(id, name.to_string())) {
            Some(info) => info.clone(),
            None => return self.err(format!("'{}' is not declared", full)),
        };
        if info.has_self {
            return self.err(format!(
                "'{}' takes 'self': call it on a value ('value.{}(...)')",
                full, name
            ));
        }
        *method.borrow_mut() = Some(MethodRef {
            module: info.module.clone(),
            key: full,
            has_self: false,
        });
        self.check_function_call_mut(&info.ty, &info.mut_params, args)
    }

    /// `person.greet()`
    fn check_method(
        &mut self,
        object_ty: &Type,
        name: &str,
        info: MethodInfo,
        args: &[AstNode],
        method: &RefCell<Option<MethodRef>>,
    ) -> CheckResult<Type> {
        let id = match object_ty {
            Type::Named { id, .. } => *id,
            _ => unreachable!("methods belong to named types"),
        };
        let full = self.method_name(id, name);
        if !info.has_self {
            return self.err(format!(
                "'{}' has no 'self': call it on the type ('{}(...)')",
                full, full
            ));
        }
        *method.borrow_mut() = Some(MethodRef {
            module: info.module.clone(),
            key: full,
            has_self: true,
        });
        let ty = match info.ty {
            Type::Function { params, ret } => Type::Function {
                params: params[1..].to_vec(),
                ret,
            },
            other => other,
        };
        self.check_function_call_mut(&ty, &info.mut_params[1..], args)
    }

    fn check_function_call(&mut self, callee_ty: &Type, args: &[AstNode]) -> CheckResult<Type> {
        self.check_function_call_mut(callee_ty, &[], args)
    }

    /// A call to a function whose parameters marked in `mut_params` are `mut`.
    fn check_function_call_mut(
        &mut self,
        callee_ty: &Type,
        mut_params: &[bool],
        args: &[AstNode],
    ) -> CheckResult<Type> {
        match callee_ty {
            Type::Function { params, ret } => {
                self.check_args_mut(params, mut_params, args)?;
                Ok((**ret).clone())
            }
            Type::Unknown => {
                for arg in args {
                    self.check_value(arg, None)?;
                }
                Ok(Type::Unknown)
            }
            other => self.err(format!("a value of type {} cannot be called", self.show(other))),
        }
    }

    fn check_args(&mut self, params: &[Type], args: &[AstNode]) -> CheckResult<Vec<Type>> {
        self.check_args_mut(params, &[], args)
    }

    fn check_args_mut(&mut self, params: &[Type], mut_params: &[bool], args: &[AstNode]) -> CheckResult<Vec<Type>> {
        if params.len() != args.len() {
            return self.err(format!(
                "expected {} argument(s), found {}",
                params.len(),
                args.len()
            ));
        }
        let mut types = Vec::new();
        for (i, (param, arg)) in params.iter().zip(args).enumerate() {
            let is_mut = mut_params.get(i).copied().unwrap_or(false);
            match (arg.unlocated(), is_mut) {
                (AstNode::MutArg(place), true) => {
                    let ty = self.check_mut_place(place)?;
                    // The function may store any value of the parameter type in the variable,
                    // so the types must be the same.
                    if !self.assignable(param, &ty) || !self.assignable(&ty, param) {
                        return self.err(format!(
                            "argument {} is 'mut' and must be exactly {}, found {}",
                            i + 1,
                            self.show(param),
                            self.show(&ty)
                        ));
                    }
                    types.push(ty);
                    continue;
                }
                (AstNode::MutArg(_), false) => {
                    return self.err(format!(
                        "argument {} is not a 'mut' parameter: remove 'mut'",
                        i + 1
                    ))
                }
                (_, true) => {
                    return self.err(format!(
                        "argument {} is a 'mut' parameter: pass a variable with 'mut' ('f(mut x)')",
                        i + 1
                    ))
                }
                _ => {}
            }
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
        let result_with = |ok: Type, error: TypeId| Type::Named {
            id: b.result,
            args: vec![ok, Self::named(error)],
        };
        let json = Self::named(b.json);
        match name {
            "print" => Ok(Type::Void),
            "panic" => {
                check(self, &[s])?;
                Ok(Type::Never)
            }
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
                    Type::String => Ok(result_with(target, b.parse_error)),
                    Type::Unknown => Ok(Type::Unknown),
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
                check(self, &[s, Self::named(b.file_mode)])?;
                Ok(result_with(Self::named(b.file), b.file_error))
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
                Ok(result_with(Type::Int, b.date_error))
            }

            "math.abs" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(types[0].clone())
            }
            "math.sqrt" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(result_with(Type::Float, b.math_error))
            }
            "math.pow" => {
                self.arity(name, &types, 2, 2)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                self.expect_numeric_arg(name, 1, &types[1])?;
                Ok(result_with(Type::Float, b.math_error))
            }
            "math.floor" | "math.ceil" | "math.round" => {
                self.arity(name, &types, 1, 1)?;
                self.expect_numeric_arg(name, 0, &types[0])?;
                Ok(Type::Int)
            }

            "http.get" | "http.delete" => {
                check(self, &[s])?;
                Ok(result_with(Self::named(b.response), b.http_error))
            }
            "http.post" | "http.put" => {
                check(self, &[s.clone(), s])?;
                Ok(result_with(Self::named(b.response), b.http_error))
            }

            "json.parse" => {
                check(self, &[s])?;
                Ok(result_with(json, b.json_error))
            }
            "json.stringify" => {
                check(self, &[json])?;
                Ok(Type::String)
            }
            "json.get" => {
                check(self, &[json.clone(), s])?;
                Ok(self.option_of(json))
            }
            "json.asString" | "json.asInt" | "json.asFloat" | "json.asBool" => {
                check(self, &[json])?;
                let value = match name {
                    "json.asString" => Type::String,
                    "json.asInt" => Type::Int,
                    "json.asFloat" => Type::Float,
                    _ => Type::Boolean,
                };
                Ok(self.option_of(value))
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
                None => return self.missing_member(*id, name),
            };
            return match symbol.kind {
                SymbolKind::Builtin(builtin) => self.check_builtin_call(&builtin, args),
                SymbolKind::Constructor { id, variant } => {
                    self.check_constructor(id, &variant, args, expected)
                }
                SymbolKind::Value => self.check_function_call_mut(&symbol.ty, &symbol.mut_params, args),
            };
        }

        // `error.message()` on standard library errors: a human-readable text.
        if let Type::Named { id, .. } | Type::Variant { id, .. } = object_ty {
            let b = self.builtins;
            let error_types = [
                b.parse_error,
                b.math_error,
                b.file_error,
                b.http_error,
                b.json_error,
                b.date_error,
            ];
            if name == "message" && error_types.contains(id) {
                self.check_args(&[], args)?;
                return Ok(Type::String);
            }
        }
        // `result.mapErr(f)`: converts the error with `f` (a function or a variant constructor).
        if let Type::Named { id, args: type_args } = object_ty {
            if *id == self.builtins.result && name == "mapErr" {
                return self.check_map_err(type_args, args);
            }
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
            Type::Unknown => {
                for arg in args {
                    self.check_value(arg, None)?;
                }
                return Ok(Type::Unknown);
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
                        self.check_pattern(arg, &Type::Unknown)?;
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

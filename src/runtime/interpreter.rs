use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use crate::error::EzError;
use crate::parser::ast::{
    AssignOperator, AstNode, BinaryOperator, ImportKind, MatchArm, Pattern, TemplatePart, TryWrap,
    TypeDef, TypeExpr, UnaryOperator,
};
use crate::runtime::env::Env;
use crate::runtime::stdlib::{self, PowFailure};
use crate::runtime::value::{format_float, Closure, ModuleValue, SumType, Value};
use crate::semantic::builtin_sums::BUILTIN_SUMS;
use crate::semantic::type_checker::{is_stdlib_import, resolve_module_path, Program};

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

pub struct Interpreter {
    program: Program,
    modules: HashMap<PathBuf, Rc<ModuleValue>>,
    stdlib: HashMap<String, Rc<ModuleValue>>,
    prelude: Rc<Env>,
    pub(crate) out: Box<dyn Write>,
    pub(crate) input: Box<dyn BufRead>,
    pub(crate) started: Instant,
    pub(crate) regex_cache: HashMap<String, regex::Regex>,
    current_module: Option<Rc<ModuleValue>>,
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
            regex_cache: HashMap::new(),
            current_module: None,
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

    fn load_module(&mut self, path: &Path) -> Result<Rc<ModuleValue>, EzError> {
        if let Some(module) = self.modules.get(path) {
            return Ok(Rc::clone(module));
        }
        let program = match self.program.modules.get(path) {
            Some(program) => Rc::clone(program),
            None => {
                return Err(EzError::ImportError(format!(
                    "module '{}' was not checked",
                    path.display()
                )))
            }
        };
        let env = Env::module(&self.prelude);
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let module = Rc::new(ModuleValue {
            name,
            env: Rc::clone(&env),
            types: RefCell::new(Vec::new()),
        });

        let display: Rc<str> = if *path == self.program.entry {
            Rc::from(
                std::env::current_dir()
                    .ok()
                    .and_then(|cwd| path.strip_prefix(cwd).ok().map(|p| p.display().to_string()))
                    .unwrap_or_else(|| path.display().to_string()),
            )
        } else {
            Rc::from(path.display().to_string())
        };
        let saved = (
            std::mem::replace(&mut self.file, display),
            std::mem::replace(
                &mut self.dir,
                path.parent().map(Path::to_path_buf).unwrap_or_default(),
            ),
            self.line,
            self.current_module.replace(Rc::clone(&module)),
        );

        let result = self.exec_statements(&program, &env);

        self.file = saved.0;
        self.dir = saved.1;
        self.line = saved.2;
        self.current_module = saved.3;

        match result {
            Ok(_) => {}
            Err(Flow::Error(error)) => return Err(error),
            Err(_) => {}
        }
        self.modules.insert(path.to_path_buf(), Rc::clone(&module));
        Ok(module)
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    /// Defines functions and constructors of a block before running it.
    fn hoist(&mut self, statements: &[AstNode], env: &Rc<Env>) -> Exec<()> {
        // Imports first, as in the type checker: the whole block can use them.
        for statement in statements {
            if let AstNode::Located { line, .. } = statement {
                self.line = *line;
            }
            if let AstNode::Import { kind, path } = statement.unlocated() {
                self.import(kind, path, env)?;
            }
        }
        for statement in statements {
            match statement.unlocated() {
                AstNode::FunctionDecl {
                    name,
                    params,
                    return_type,
                    body,
                } => {
                    let closure = Closure {
                        name: name.clone(),
                        params: params.clone(),
                        body: Rc::clone(body),
                        env: Rc::clone(env),
                        float_params: params.iter().map(|p| is_float_type(&p.ty)).collect(),
                        returns_float: is_float_type(return_type),
                        file: Rc::clone(&self.file),
                    };
                    env.define(name, Value::Function(Rc::new(closure)));
                }
                AstNode::TypeDecl {
                    name,
                    definition: TypeDef::Sum(variants),
                } => {
                    let sum = Rc::new(SumType {
                        name: name.clone(),
                        variants: variants
                            .iter()
                            .map(|v| {
                                (v.name.clone(), v.fields.iter().map(|f| f.name.clone()).collect())
                            })
                            .collect(),
                    });
                    for variant in variants {
                        env.define(&variant.name, sum.constructor(&variant.name));
                    }
                    if let Some(module) = &self.current_module {
                        if Rc::ptr_eq(&module.env, env) {
                            module.types.borrow_mut().push(sum);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn exec_statements(&mut self, statements: &[AstNode], env: &Rc<Env>) -> Exec<Value> {
        self.hoist(statements, env)?;
        let mut last = Value::Void;
        for statement in statements {
            last = self.eval(statement, env)?;
        }
        match statements.last().map(AstNode::unlocated) {
            Some(
                AstNode::While { .. }
                | AstNode::For { .. }
                | AstNode::Assign { .. }
                | AstNode::VarDecl { .. }
                | AstNode::FunctionDecl { .. }
                | AstNode::TypeDecl { .. }
                | AstNode::Import { .. },
            ) => Ok(Value::Void),
            _ => Ok(last),
        }
    }

    /// Runs a block (or a single expression, e.g. a match arm) in a new scope.
    fn exec_block(&mut self, node: &AstNode, env: &Rc<Env>) -> Exec<Value> {
        let scope = Env::child(env);
        match node.unlocated() {
            AstNode::Block(statements) => self.exec_statements(statements, &scope),
            other => self.eval(other, &scope),
        }
    }

    fn exec_loop_body(&mut self, body: &AstNode, env: &Rc<Env>) -> Exec<bool> {
        match self.exec_block(body, env) {
            Ok(_) | Err(Flow::Continue) => Ok(true),
            Err(Flow::Break) => Ok(false),
            Err(other) => Err(other),
        }
    }

    fn import(&mut self, kind: &ImportKind, path: &str, env: &Rc<Env>) -> Exec<()> {
        let module = if is_stdlib_import(path) {
            self.stdlib_module(path)
        } else {
            let resolved = resolve_module_path(&self.dir, path);
            self.load_module(&resolved)?
        };
        match kind {
            ImportKind::Module(name) => env.define(name, Value::Module(module)),
            ImportKind::Names(names) => {
                for name in names {
                    // Imported names always read the module's variable (see `Env::alias`).
                    if module.env.get_local(name).is_some() {
                        env.alias(name, &module.env);
                    }
                    // Importing a sum type also imports its variants. Shapes and aliases have
                    // no runtime value.
                    let types = module.types.borrow();
                    if let Some(sum) = types.iter().find(|sum| sum.name == *name) {
                        for (variant, _) in &sum.variants {
                            env.alias(variant, &module.env);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn stdlib_module(&mut self, name: &str) -> Rc<ModuleValue> {
        if let Some(module) = self.stdlib.get(name) {
            return Rc::clone(module);
        }
        let env = Env::new(None);
        for (member, value) in stdlib::module_members(name) {
            env.define(&member, value);
        }
        let types = BUILTIN_SUMS
            .iter()
            .filter(|sum| sum.module == Some(name))
            .map(|sum| stdlib::sum(sum.name))
            .collect();
        let module = Rc::new(ModuleValue {
            name: name.to_string(),
            env,
            types: RefCell::new(types),
        });
        self.stdlib.insert(name.to_string(), Rc::clone(&module));
        module
    }

    // ------------------------------------------------------------------
    // Evaluation
    // ------------------------------------------------------------------

    pub fn eval(&mut self, node: &AstNode, env: &Rc<Env>) -> Exec<Value> {
        match node {
            AstNode::Located { line, node } => {
                self.line = *line;
                self.eval(node, env)
            }
            AstNode::Export(node) => self.eval(node, env),
            AstNode::Try { value, wrap } => self.eval_try(value, wrap, env),
            AstNode::IntLiteral(n) => Ok(Value::Int(*n)),
            AstNode::FloatLiteral(n) => Ok(Value::Float(*n)),
            AstNode::StringLiteral(s) => Ok(Value::str(s)),
            AstNode::BooleanLiteral(b) => Ok(Value::Bool(*b)),
            AstNode::TemplateString(parts) => {
                let mut text = String::new();
                for part in parts {
                    match part {
                        TemplatePart::Text(s) => text.push_str(s),
                        TemplatePart::Expression(expression) => {
                            text.push_str(&self.eval(expression, env)?.display())
                        }
                    }
                }
                Ok(Value::str(&text))
            }
            AstNode::ArrayLiteral(elements) => {
                let mut items = Vec::with_capacity(elements.len());
                for element in elements {
                    items.push(self.eval(element, env)?);
                }
                Ok(Value::list(items))
            }
            AstNode::ObjectLiteral(entries) => {
                let mut fields = Vec::with_capacity(entries.len());
                for entry in entries {
                    fields.push((entry.key.clone(), self.eval(&entry.value, env)?));
                }
                Ok(Value::object(fields))
            }
            AstNode::Identifier(name) => match env.get(name) {
                Some(value) => Ok(value),
                None => Err(self.error(format!("undefined variable '{}'", name))),
            },
            AstNode::Unary { op, operand } => {
                let value = self.eval(operand, env)?;
                self.unary(op, value)
            }
            AstNode::BinaryOp { left, op, right } => self.eval_binary(left, *op, right, env),
            AstNode::Call { callee, args } => self.eval_call(callee, args, env),
            AstNode::Index { target, index } => {
                let target = self.eval(target, env)?;
                let index = self.eval(index, env)?;
                self.index(&target, &index)
            }
            AstNode::Member { object, name } => {
                let object = self.eval(object, env)?;
                self.member(&object, name)
            }
            AstNode::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.condition(condition, env)? {
                    let value = self.exec_block(then_branch, env)?;
                    Ok(if else_branch.is_some() { value } else { Value::Void })
                } else if let Some(else_branch) = else_branch {
                    self.exec_block(else_branch, env)
                } else {
                    Ok(Value::Void)
                }
            }
            AstNode::Match { scrutinee, arms } => {
                let value = self.eval(scrutinee, env)?;
                self.eval_match(&value, arms, env)
            }
            AstNode::Lambda {
                params,
                return_type,
                body,
            } => Ok(Value::Function(Rc::new(Closure {
                name: "anonymous function".to_string(),
                params: params.clone(),
                body: Rc::clone(body),
                env: Env::snapshot(env),
                float_params: params.iter().map(|p| is_float_type(&p.ty)).collect(),
                returns_float: is_float_type(return_type),
                file: Rc::clone(&self.file),
            }))),
            AstNode::Block(_) => self.exec_block(node, env),

            AstNode::While { condition, body } => {
                while self.condition(condition, env)? {
                    if !self.exec_loop_body(body, env)? {
                        break;
                    }
                }
                Ok(Value::Void)
            }
            AstNode::For {
                variable,
                iterable,
                body,
            } => {
                let iterable = self.eval(iterable, env)?;
                let items: Vec<Value> = match &iterable {
                    Value::List(items) => items.borrow().clone(),
                    Value::Str(s) => s.chars().map(|c| Value::str(&c.to_string())).collect(),
                    Value::Object(object) => object
                        .borrow()
                        .fields
                        .iter()
                        .map(|(k, _)| Value::str(k))
                        .collect(),
                    other => {
                        return Err(self.error(format!(
                            "cannot iterate over a value of type {}",
                            other.type_name()
                        )))
                    }
                };
                for item in items {
                    let scope = Env::child(env);
                    scope.define(variable, item);
                    if !self.exec_loop_body(body, &scope)? {
                        break;
                    }
                }
                Ok(Value::Void)
            }
            AstNode::Break => Err(Flow::Break),
            AstNode::Continue => Err(Flow::Continue),
            AstNode::Return(value) => {
                let value = match value {
                    Some(value) => self.eval(value, env)?,
                    None => Value::Void,
                };
                Err(Flow::Return(value))
            }
            AstNode::Assign { target, op, value } => {
                self.eval_assign(target, op, value, env)?;
                Ok(Value::Void)
            }
            AstNode::VarDecl { name, ty, value, .. } => {
                let value = self.eval(value, env)?;
                env.define(name, coerce(value, is_float_type(ty)));
                Ok(Value::Void)
            }
            // A function declared inside another function captures a copy of the variables
            // around it where it is declared (it was hoisted with access to the whole block).
            AstNode::FunctionDecl {
                name,
                params,
                return_type,
                body,
            } => {
                let captured = Env::snapshot(env);
                if !Rc::ptr_eq(&captured, env) {
                    env.define(
                        name,
                        Value::Function(Rc::new(Closure {
                            name: name.clone(),
                            params: params.clone(),
                            body: Rc::clone(body),
                            env: captured,
                            float_params: params.iter().map(|p| is_float_type(&p.ty)).collect(),
                            returns_float: is_float_type(return_type),
                            file: Rc::clone(&self.file),
                        })),
                    );
                }
                Ok(Value::Void)
            }
            AstNode::TypeDecl { .. } => Ok(Value::Void),
            // Imports run when the block is hoisted.
            AstNode::Import { .. } => Ok(Value::Void),
        }
    }

    /// `value?`: the `Ok`/`Some` value, or an early return of the `Err`/`None`. At the top level
    /// of the main file, the failure stops the program.
    fn eval_try(&mut self, value: &AstNode, wrap: &RefCell<Option<TryWrap>>, env: &Rc<Env>) -> Exec<Value> {
        let variant = match self.eval(value, env)? {
            Value::Variant(variant) => variant,
            other => {
                return Err(self.error(format!(
                    "'?' expects a Result or an Option, found {}",
                    other.type_name()
                )))
            }
        };
        let failure = match variant.name.as_str() {
            "Ok" | "Some" => return Ok(variant.values[0].clone()),
            "None" => stdlib::none(),
            _ => {
                let error = variant.values[0].clone();
                let error = match &*wrap.borrow() {
                    // Automatic conversion: wrap the error in the variant the checker chose.
                    Some(wrap) => {
                        let constructor = match &wrap.module {
                            None => env.get(&wrap.variant),
                            Some(module) => match env.get(module) {
                                Some(Value::Module(module)) => module.env.get_local(&wrap.variant),
                                _ => None,
                            },
                        };
                        match constructor {
                            Some(constructor) => self.call_value(&constructor, vec![error])?,
                            None => {
                                return Err(self.error(format!("'{}' is not in scope", wrap.variant)))
                            }
                        }
                    }
                    None => error,
                };
                stdlib::err(error)
            }
        };
        if self.function_depth == 0 {
            // The error itself, readable (`file 'a.txt' not found`), or `None`.
            let shown = match &failure {
                Value::Variant(variant) if variant.name == "Err" => variant.values[0].display(),
                other => other.display(),
            };
            return Err(Flow::Error(EzError::UnhandledError(format!(
                "{}: {}",
                self.location(),
                shown
            ))));
        }
        Err(Flow::Return(failure))
    }

    fn condition(&mut self, condition: &AstNode, env: &Rc<Env>) -> Exec<bool> {
        match self.eval(condition, env)? {
            Value::Bool(b) => Ok(b),
            other => Err(self.error(format!(
                "condition must be boolean, found {}",
                other.type_name()
            ))),
        }
    }

    fn eval_assign(
        &mut self,
        target: &AstNode,
        op: &AssignOperator,
        value: &AstNode,
        env: &Rc<Env>,
    ) -> Exec<()> {
        let new_value = |interpreter: &mut Self, current: Option<Value>| -> Exec<Value> {
            let value = interpreter.eval(value, env)?;
            match (op.binary(), current) {
                (None, Some(Value::Float(_))) => Ok(coerce(value, true)),
                (None, _) => Ok(value),
                (Some(binary), Some(current)) => {
                    let is_float = matches!(current, Value::Float(_));
                    Ok(coerce(interpreter.binary(binary, current, value)?, is_float))
                }
                (Some(_), None) => Err(interpreter.error("compound assignment to an undefined value")),
            }
        };

        match target.unlocated() {
            AstNode::Identifier(name) => {
                let current = env.get(name);
                let value = new_value(self, current)?;
                if !env.assign(name, value.clone()) {
                    env.define(name, value);
                }
                Ok(())
            }
            AstNode::Index { target, index } => {
                let container = self.eval(target, env)?;
                let index = self.eval(index, env)?;
                match &container {
                    Value::List(items) => {
                        let position = self.list_position(items.borrow().len(), &index)?;
                        let current = items.borrow()[position].clone();
                        let value = new_value(self, Some(current))?;
                        items.borrow_mut()[position] = value;
                        Ok(())
                    }
                    Value::Object(object) => {
                        let key = match &index {
                            Value::Str(key) => key.to_string(),
                            other => {
                                return Err(self.error(format!(
                                    "dictionary keys must be strings, found {}",
                                    other.type_name()
                                )))
                            }
                        };
                        let current = object.borrow().get(&key).cloned();
                        let value = if op.binary().is_some() && current.is_none() {
                            return Err(self.error(format!("key '{}' not found", key)));
                        } else {
                            new_value(self, current)?
                        };
                        object.borrow_mut().set(&key, value);
                        Ok(())
                    }
                    other => Err(self.error(format!(
                        "cannot assign by index to a value of type {}",
                        other.type_name()
                    ))),
                }
            }
            AstNode::Member { object, name } => {
                let object = self.eval(object, env)?;
                match &object {
                    Value::Object(fields) => {
                        let current = fields.borrow().get(name).cloned();
                        let value = new_value(self, current)?;
                        fields.borrow_mut().set(name, value);
                        Ok(())
                    }
                    other => Err(self.error(format!(
                        "cannot assign field '{}' on a value of type {}",
                        name,
                        other.type_name()
                    ))),
                }
            }
            _ => Err(self.error("invalid assignment target")),
        }
    }

    fn list_position(&self, len: usize, index: &Value) -> Exec<usize> {
        match index {
            Value::Int(i) if *i >= 0 && (*i as usize) < len => Ok(*i as usize),
            Value::Int(i) => Err(self.error(format!(
                "index {} out of range (length {})",
                i, len
            ))),
            other => Err(self.error(format!("index must be int, found {}", other.type_name()))),
        }
    }

    fn index(&self, target: &Value, index: &Value) -> Exec<Value> {
        match target {
            Value::List(items) => {
                let items = items.borrow();
                let position = self.list_position(items.len(), index)?;
                Ok(items[position].clone())
            }
            Value::Str(s) => {
                let chars: Vec<char> = s.chars().collect();
                let position = self.list_position(chars.len(), index)?;
                Ok(Value::str(&chars[position].to_string()))
            }
            Value::Object(object) => match index {
                Value::Str(key) => match object.borrow().get(key) {
                    Some(value) => Ok(value.clone()),
                    None => Err(self.error(format!("key '{}' not found", key))),
                },
                Value::Int(_) => Err(self.error("objects cannot be indexed by int")),
                other => Err(self.error(format!(
                    "dictionary keys must be strings, found {}",
                    other.type_name()
                ))),
            },
            other => Err(self.error(format!("cannot index a value of type {}", other.type_name()))),
        }
    }

    fn member(&self, object: &Value, name: &str) -> Exec<Value> {
        match object {
            Value::Object(fields) => match fields.borrow().get(name) {
                Some(value) => Ok(value.clone()),
                None => Err(self.error(format!("field '{}' not found", name))),
            },
            Value::Variant(variant) => match variant.field(name) {
                Some(value) => Ok(value.clone()),
                None => Err(self.error(format!(
                    "variant '{}' has no field '{}'",
                    variant.name, name
                ))),
            },
            Value::Module(module) => match module.env.get_local(name) {
                Some(value) => Ok(value),
                None => Err(self.error(format!("module '{}' has no member '{}'", module.name, name))),
            },
            Value::Native(native) => match &*native.borrow() {
                crate::runtime::value::Native::DateTime(datetime) => match datetime.field(name) {
                    Some(n) => Ok(Value::Int(n)),
                    None => Err(self.error(format!("DateTime has no field '{}'", name))),
                },
                _ => Err(self.error(format!("'{}' must be called", name))),
            },
            other => Err(self.error(format!(
                "a value of type {} has no field '{}'",
                other.type_name(),
                name
            ))),
        }
    }

    // ------------------------------------------------------------------
    // Operators
    // ------------------------------------------------------------------

    fn unary(&self, op: &UnaryOperator, value: Value) -> Exec<Value> {
        match (op, value) {
            (UnaryOperator::Negate, Value::Int(n)) => match n.checked_neg() {
                Some(n) => Ok(Value::Int(n)),
                None => Err(self.error("integer overflow")),
            },
            (UnaryOperator::Negate, Value::Float(n)) => Ok(Value::Float(-n)),
            (UnaryOperator::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOperator::BitNot, Value::Int(n)) => Ok(Value::Int(!n)),
            (_, value) => Err(self.error(format!(
                "invalid operand of type {} for a unary operator",
                value.type_name()
            ))),
        }
    }

    fn eval_binary(
        &mut self,
        left: &AstNode,
        op: BinaryOperator,
        right: &AstNode,
        env: &Rc<Env>,
    ) -> Exec<Value> {
        match op {
            BinaryOperator::And | BinaryOperator::Or => {
                let left_value = self.condition(left, env)?;
                if (op == BinaryOperator::And) != left_value {
                    return Ok(Value::Bool(left_value));
                }
                Ok(Value::Bool(self.condition(right, env)?))
            }
            BinaryOperator::Is => {
                let left_value = self.eval(left, env)?;
                let right_value = self.eval(right, env)?;
                // `value is Some`: variant test.
                if let AstNode::Identifier(name) = right.unlocated() {
                    let is_variant_name = match &right_value {
                        Value::Constructor(constructor) => constructor.variant == *name,
                        Value::Variant(variant) => variant.name == *name && variant.values.is_empty(),
                        _ => false,
                    };
                    if is_variant_name {
                        return Ok(Value::Bool(match &left_value {
                            Value::Variant(variant) => variant.name == *name,
                            _ => false,
                        }));
                    }
                }
                Ok(Value::Bool(left_value.identical(&right_value)))
            }
            _ => {
                let left = self.eval(left, env)?;
                let right = self.eval(right, env)?;
                self.binary(op, left, right)
            }
        }
    }

    pub(crate) fn binary(&self, op: BinaryOperator, left: Value, right: Value) -> Exec<Value> {
        use BinaryOperator::*;
        let overflow = || self.error("integer overflow");
        match (op, &left, &right) {
            (Equal, _, _) => Ok(Value::Bool(left.equals(&right))),
            (NotEqual, _, _) => Ok(Value::Bool(!left.equals(&right))),
            (Is, _, _) => Ok(Value::Bool(left.identical(&right))),
            (In, _, _) => self.contains(&right, &left).map(Value::Bool),

            (Add, Value::Int(a), Value::Int(b)) => a.checked_add(*b).map(Value::Int).ok_or_else(overflow),
            (Subtract, Value::Int(a), Value::Int(b)) => a.checked_sub(*b).map(Value::Int).ok_or_else(overflow),
            (Multiply, Value::Int(a), Value::Int(b)) => a.checked_mul(*b).map(Value::Int).ok_or_else(overflow),
            // Like Go and Rust: integer division truncates toward zero and the remainder has
            // the sign of the dividend, so `a == (a / b) * b + a % b`.
            (Divide | Modulo, Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(self.error("division by zero"));
                }
                let result = if op == Divide {
                    a.checked_div(*b)
                } else {
                    a.checked_rem(*b)
                };
                result.map(Value::Int).ok_or_else(overflow)
            }
            (Power, Value::Int(a), Value::Int(b)) => {
                if *a == 0 && *b < 0 {
                    return Err(self.error("division by zero"));
                }
                if *b < 0 {
                    return Err(self.error("negative exponent with int operands; use a float base"));
                }
                let exponent = u32::try_from(*b).map_err(|_| overflow())?;
                a.checked_pow(exponent).map(Value::Int).ok_or_else(overflow)
            }
            (
                Add | Subtract | Multiply | Divide | Modulo | Power,
                Value::Int(_) | Value::Float(_),
                Value::Int(_) | Value::Float(_),
            ) => {
                let (a, b) = (as_float(&left), as_float(&right));
                let result = match op {
                    Add => a + b,
                    Subtract => a - b,
                    Multiply => a * b,
                    // Operators cannot return a Result: every failure panics.
                    Power => match stdlib::real_pow(a, b) {
                        Ok(result) => result,
                        Err(PowFailure::Complex) => {
                            return Err(self.error(format!(
                                "{} ** {} is not a real number (use math.pow to handle it)",
                                format_float(a),
                                format_float(b)
                            )))
                        }
                        // `0 ** -1` is `1 / 0`.
                        Err(PowFailure::Infinite) => return Err(self.error("division by zero")),
                        Err(PowFailure::Overflow) => return Err(self.error("float overflow")),
                    },
                    _ => {
                        if b == 0.0 {
                            return Err(self.error("division by zero"));
                        }
                        if op == Divide {
                            a / b
                        } else {
                            a % b
                        }
                    }
                };
                // `float` has no infinity: a result that does not fit is a representation limit.
                if !result.is_finite() {
                    return Err(self.error("float overflow"));
                }
                Ok(Value::Float(result))
            }
            (Add, Value::Str(a), Value::Str(b)) => Ok(Value::str(&format!("{}{}", a, b))),
            (Multiply, Value::Str(s), Value::Int(n)) | (Multiply, Value::Int(n), Value::Str(s)) => {
                Ok(Value::str(&s.repeat((*n).max(0) as usize)))
            }
            (Add, Value::List(a), Value::List(b)) => {
                let mut items = a.borrow().clone();
                items.extend(b.borrow().iter().cloned());
                Ok(Value::list(items))
            }

            (Greater | GreaterEqual | Less | LessEqual, _, _) => {
                let ordering = match (&left, &right) {
                    (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
                    (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
                        as_float(&left).partial_cmp(&as_float(&right))
                    }
                    (Value::Str(a), Value::Str(b)) => a.partial_cmp(b),
                    _ => {
                        return Err(self.error(format!(
                            "cannot compare {} with {}",
                            left.type_name(),
                            right.type_name()
                        )))
                    }
                };
                let result = match ordering {
                    Some(ordering) => match op {
                        Greater => ordering.is_gt(),
                        GreaterEqual => ordering.is_ge(),
                        Less => ordering.is_lt(),
                        _ => ordering.is_le(),
                    },
                    None => false,
                };
                Ok(Value::Bool(result))
            }

            (BitAnd, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a & b)),
            (BitOr, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a | b)),
            (BitXor, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a ^ b)),
            (ShiftLeft | ShiftRight, Value::Int(a), Value::Int(b)) => {
                if !(0..64).contains(b) {
                    return Err(self.error(format!("invalid shift amount {}", b)));
                }
                Ok(Value::Int(if op == ShiftLeft { a << b } else { a >> b }))
            }
            (And | Or, Value::Bool(a), Value::Bool(b)) => {
                Ok(Value::Bool(if op == And { *a && *b } else { *a || *b }))
            }
            _ => Err(self.error(format!(
                "operator '{}' cannot be applied to {} and {}",
                op.symbol(),
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    /// `item in collection`
    fn contains(&self, collection: &Value, item: &Value) -> Exec<bool> {
        match (collection, item) {
            (Value::Str(haystack), Value::Str(needle)) => Ok(haystack.contains(&**needle)),
            (Value::List(items), _) => Ok(items.borrow().iter().any(|v| v.equals(item))),
            (Value::Object(object), Value::Str(key)) => Ok(object.borrow().get(key).is_some()),
            _ => Err(self.error(format!(
                "'in' cannot check a {} inside a {}",
                item.type_name(),
                collection.type_name()
            ))),
        }
    }

    // ------------------------------------------------------------------
    // Calls
    // ------------------------------------------------------------------

    fn eval_call(&mut self, callee: &AstNode, args: &[AstNode], env: &Rc<Env>) -> Exec<Value> {
        if let AstNode::Member { object, name } = callee.unlocated() {
            let object = self.eval(object, env)?;
            let args = self.eval_args(args, env)?;
            return self.call_method(&object, name, args);
        }
        let callee = self.eval(callee, env)?;
        let args = self.eval_args(args, env)?;
        self.call_value(&callee, args)
    }

    fn eval_args(&mut self, args: &[AstNode], env: &Rc<Env>) -> Exec<Vec<Value>> {
        args.iter().map(|arg| self.eval(arg, env)).collect()
    }

    pub(crate) fn call_value(&mut self, callee: &Value, args: Vec<Value>) -> Exec<Value> {
        match callee {
            Value::Function(closure) => self.call_closure(closure, args),
            Value::Builtin(name) => stdlib::call(self, name, args),
            Value::Constructor(constructor) => {
                if args.len() != constructor.arity() {
                    return Err(self.error(format!(
                        "'{}' expects {} argument(s), found {}",
                        constructor.variant,
                        constructor.arity(),
                        args.len()
                    )));
                }
                Ok(constructor.sum.make(&constructor.variant, args))
            }
            other => Err(self.error(format!("a value of type {} cannot be called", other.type_name()))),
        }
    }

    fn call_closure(&mut self, closure: &Rc<Closure>, args: Vec<Value>) -> Exec<Value> {
        if args.len() != closure.params.len() {
            return Err(self.error(format!(
                "'{}' expects {} argument(s), found {}",
                closure.name,
                closure.params.len(),
                args.len()
            )));
        }
        let scope = Env::child(&closure.env);
        for ((param, value), to_float) in closure.params.iter().zip(args).zip(&closure.float_params) {
            scope.define(&param.name, coerce(value, *to_float));
        }

        let saved_file = std::mem::replace(&mut self.file, Rc::clone(&closure.file));
        let saved_line = self.line;
        self.function_depth += 1;
        let result = match closure.body.unlocated() {
            AstNode::Block(statements) => self.exec_statements(statements, &scope),
            expression => self.eval(expression, &scope),
        };
        self.function_depth -= 1;
        self.file = saved_file;
        self.line = saved_line;

        let value = match result {
            Ok(value) | Err(Flow::Return(value)) => value,
            Err(Flow::Break) | Err(Flow::Continue) => Value::Void,
            Err(error) => return Err(error),
        };
        Ok(coerce(value, closure.returns_float))
    }

    fn call_method(&mut self, object: &Value, name: &str, args: Vec<Value>) -> Exec<Value> {
        match object {
            Value::Module(_) => {
                let member = self.member(object, name)?;
                self.call_value(&member, args)
            }
            Value::Str(s) => stdlib::string_method(self, s, name, args),
            Value::List(items) => match name {
                "length" => Ok(Value::Int(items.borrow().len() as i64)),
                _ => Err(self.error(format!("lists have no method '{}'", name))),
            },
            Value::Object(fields) => {
                if name == "length" && fields.borrow().get("length").is_none() {
                    return Ok(Value::Int(fields.borrow().fields.len() as i64));
                }
                let member = self.member(object, name)?;
                self.call_value(&member, args)
            }
            Value::Native(native) => stdlib::native_method(self, native, name, args),
            Value::Variant(variant) if name == "message" => match stdlib::error_message(variant) {
                Some(message) => Ok(Value::str(&message)),
                None => Err(self.error(format!("'{}' has no method 'message'", variant.sum.name))),
            },
            Value::Variant(variant) if variant.sum.name == "Result" && name == "mapErr" => {
                if variant.name != "Err" {
                    return Ok(object.clone());
                }
                let converter = args.into_iter().next().unwrap_or(Value::Void);
                let error = self.call_value(&converter, vec![variant.values[0].clone()])?;
                Ok(stdlib::err(error))
            }
            other => Err(self.error(format!(
                "a value of type {} has no method '{}'",
                other.type_name(),
                name
            ))),
        }
    }

    // ------------------------------------------------------------------
    // Match
    // ------------------------------------------------------------------

    fn eval_match(&mut self, value: &Value, arms: &[MatchArm], env: &Rc<Env>) -> Exec<Value> {
        for arm in arms {
            let scope = Env::child(env);
            if !self.match_pattern(&arm.pattern, value, &scope)? {
                continue;
            }
            if let Some(guard) = &arm.guard {
                if !self.condition(guard, &scope)? {
                    continue;
                }
            }
            return self.exec_block(&arm.body, &scope);
        }
        Err(self.error(format!("no match arm matches the value {}", value.repr())))
    }

    /// Whether `name` in a pattern names a variant (rather than introducing a binding).
    fn is_variant_name(env: &Rc<Env>, name: &str) -> bool {
        match env.get(name) {
            Some(Value::Constructor(constructor)) => constructor.variant == name,
            Some(Value::Variant(variant)) => variant.name == name && variant.values.is_empty(),
            _ => false,
        }
    }

    fn match_pattern(&mut self, pattern: &Pattern, value: &Value, env: &Rc<Env>) -> Exec<bool> {
        match pattern {
            Pattern::Wildcard => Ok(true),
            Pattern::Identifier(name) => {
                // A name is a variant when the matched value's type has it (`Null` against a
                // `Json`), even if the variant is not in scope.
                let is_variant = match value {
                    Value::Variant(variant) => variant.sum.has_variant(name),
                    _ => false,
                } || Self::is_variant_name(env, name);
                if is_variant {
                    return Ok(matches!(value, Value::Variant(v) if v.name == *name));
                }
                env.define(name, value.clone());
                Ok(true)
            }
            Pattern::Variant { name, args } => match value {
                Value::Variant(variant) if variant.name == *name && variant.values.len() == args.len() => {
                    for (sub_pattern, sub_value) in args.iter().zip(&variant.values) {
                        if !self.match_pattern(sub_pattern, sub_value, env)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
            Pattern::Literal(literal) => {
                let literal = self.eval(literal, env)?;
                Ok(literal.equals(value))
            }
        }
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

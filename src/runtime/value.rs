use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::parser::ast::{AstNode, Param};
use crate::runtime::env::Env;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    Bool(bool),
    Void,
    /// Lists are reference values: a `const` list can still have its contents changed.
    List(Rc<RefCell<Vec<Value>>>),
    /// Shape-typed values, object literals and dictionaries.
    Object(Rc<RefCell<Object>>),
    Variant(Rc<VariantValue>),
    Function(Rc<Closure>),
    /// Built-in function, by qualified name (`print`, `math.sqrt`).
    Builtin(Rc<str>),
    Constructor(Rc<Constructor>),
    Module(Rc<ModuleValue>),
    Native(Rc<RefCell<Native>>),
}

#[derive(Debug, Default)]
pub struct Object {
    /// Fields in declaration order.
    pub fields: Vec<(String, Value)>,
}

impl Object {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn set(&mut self, name: &str, value: Value) {
        match self.fields.iter_mut().find(|(n, _)| n == name) {
            Some((_, slot)) => *slot = value,
            None => self.fields.push((name.to_string(), value)),
        }
    }
}

/// A sum type known at runtime: its name and its variants, with their field names.
#[derive(Debug)]
pub struct SumType {
    pub name: String,
    pub variants: Vec<(String, Vec<String>)>,
}

impl SumType {
    pub fn has_variant(&self, variant: &str) -> bool {
        self.variants.iter().any(|(name, _)| name == variant)
    }

    pub fn fields(&self, variant: &str) -> &[String] {
        self.variants
            .iter()
            .find(|(name, _)| name == variant)
            .map(|(_, fields)| fields.as_slice())
            .unwrap_or(&[])
    }

    /// The value bound to a variant name: the value itself for a variant without fields,
    /// a constructor otherwise.
    pub fn constructor(self: &Rc<Self>, variant: &str) -> Value {
        if self.fields(variant).is_empty() {
            self.make(variant, vec![])
        } else {
            Value::Constructor(Rc::new(Constructor {
                sum: Rc::clone(self),
                variant: variant.to_string(),
            }))
        }
    }

    pub fn make(self: &Rc<Self>, variant: &str, values: Vec<Value>) -> Value {
        Value::Variant(Rc::new(VariantValue {
            sum: Rc::clone(self),
            name: variant.to_string(),
            values,
        }))
    }
}

#[derive(Debug)]
pub struct VariantValue {
    pub sum: Rc<SumType>,
    pub name: String,
    pub values: Vec<Value>,
}

impl VariantValue {
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.sum
            .fields(&self.name)
            .iter()
            .position(|n| n == name)
            .map(|i| &self.values[i])
    }
}

#[derive(Debug)]
pub struct Closure {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Rc<AstNode>,
    pub env: Rc<Env>,
    /// Parameters declared as `float` (integers passed to them are converted).
    pub float_params: Vec<bool>,
    pub returns_float: bool,
    /// File where the function was defined (for error messages).
    pub file: Rc<str>,
}

#[derive(Debug)]
pub struct Constructor {
    pub sum: Rc<SumType>,
    pub variant: String,
}

impl Constructor {
    pub fn arity(&self) -> usize {
        self.sum.fields(&self.variant).len()
    }
}

#[derive(Debug)]
pub struct ModuleValue {
    pub name: String,
    pub env: Rc<Env>,
    /// Sum types declared at the top level of the module.
    pub types: RefCell<Vec<Rc<SumType>>>,
}

#[derive(Debug)]
pub enum Native {
    File(FileHandle),
    DateTime(DateTime),
    Regex(regex::Regex),
}

#[derive(Debug)]
pub struct FileHandle {
    pub path: String,
    pub mode: String,
    pub file: Option<std::fs::File>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DateTime {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
}

impl DateTime {
    pub fn field(&self, name: &str) -> Option<i64> {
        Some(match name {
            "year" => self.year,
            "month" => self.month,
            "day" => self.day,
            "hour" => self.hour,
            "minute" => self.minute,
            "second" => self.second,
            _ => return None,
        })
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

pub fn format_float(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else if value.is_infinite() {
        if value > 0.0 { "inf" } else { "-inf" }.to_string()
    } else if value.abs() >= 1e16 {
        // Large values in scientific notation, so they never look like an int.
        format!("{:e}", value)
    } else if value.fract() == 0.0 {
        format!("{:.1}", value)
    } else {
        format!("{}", value)
    }
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }

    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(RefCell::new(items)))
    }

    pub fn object(fields: Vec<(String, Value)>) -> Value {
        Value::Object(Rc::new(RefCell::new(Object { fields })))
    }

    /// A copy that shares nothing with the original (lists and objects are copied too).
    pub fn deep_copy(&self) -> Value {
        match self {
            Value::List(items) => Value::list(items.borrow().iter().map(Value::deep_copy).collect()),
            Value::Object(object) => Value::object(
                object
                    .borrow()
                    .fields
                    .iter()
                    .map(|(name, value)| (name.clone(), value.deep_copy()))
                    .collect(),
            ),
            Value::Variant(variant) => Value::Variant(Rc::new(VariantValue {
                sum: Rc::clone(&variant.sum),
                name: variant.name.clone(),
                values: variant.values.iter().map(Value::deep_copy).collect(),
            })),
            other => other.clone(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::Bool(_) => "boolean",
            Value::Void => "void",
            Value::List(_) => "list",
            Value::Object(_) => "object",
            Value::Variant(_) => "variant",
            Value::Function(_) | Value::Builtin(_) | Value::Constructor(_) => "function",
            Value::Module(_) => "module",
            Value::Native(native) => match &*native.borrow() {
                Native::File(_) => "File",
                Native::DateTime(_) => "DateTime",
                Native::Regex(_) => "Regex",
            },
        }
    }

    /// Text shown by `print` and string interpolation. Standard library errors show a
    /// human-readable message.
    pub fn display(&self) -> String {
        match self {
            Value::Str(s) => s.to_string(),
            Value::Variant(variant) => crate::runtime::stdlib::error_message(variant)
                .unwrap_or_else(|| self.repr()),
            other => other.repr(),
        }
    }

    /// Text of a value nested in another one (strings are quoted).
    pub fn repr(&self) -> String {
        let mut out = String::new();
        self.write_repr(&mut out);
        out
    }

    fn write_repr(&self, out: &mut String) {
        match self {
            Value::Int(n) => {
                let _ = write!(out, "{}", n);
            }
            Value::Float(f) => out.push_str(&format_float(*f)),
            Value::Str(s) => {
                out.push('\'');
                for c in s.chars() {
                    match c {
                        '\'' => out.push_str("\\'"),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\t' => out.push_str("\\t"),
                        '\r' => out.push_str("\\r"),
                        other => out.push(other),
                    }
                }
                out.push('\'');
            }
            Value::Bool(b) => {
                let _ = write!(out, "{}", b);
            }
            Value::Void => out.push_str("void"),
            Value::List(items) => {
                out.push('[');
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.write_repr(out);
                }
                out.push(']');
            }
            Value::Object(object) => {
                let object = object.borrow();
                if object.fields.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{ ");
                for (i, (name, value)) in object.fields.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    if is_identifier(name) {
                        out.push_str(name);
                    } else {
                        Value::str(name).write_repr(out);
                    }
                    out.push_str(": ");
                    value.write_repr(out);
                }
                out.push_str(" }");
            }
            Value::Variant(variant) => {
                out.push_str(&variant.name);
                if !variant.values.is_empty() {
                    out.push('(');
                    for (i, value) in variant.values.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        value.write_repr(out);
                    }
                    out.push(')');
                }
            }
            Value::Function(closure) => {
                let _ = write!(out, "<function {}>", closure.name);
            }
            Value::Builtin(name) => {
                let _ = write!(out, "<built-in function {}>", name);
            }
            Value::Constructor(constructor) => {
                let _ = write!(out, "<constructor {}>", constructor.variant);
            }
            Value::Module(module) => {
                let _ = write!(out, "<module {}>", module.name);
            }
            Value::Native(native) => match &*native.borrow() {
                Native::File(file) => {
                    let _ = write!(out, "<file '{}'>", file.path);
                }
                Native::DateTime(datetime) => {
                    let _ = write!(out, "{}", datetime);
                }
                Native::Regex(regex) => {
                    let _ = write!(out, "<regex '{}'>", regex.as_str());
                }
            },
        }
    }

    /// Structural equality (`==`).
    pub fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => *a as f64 == *b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Void, Value::Void) => true,
            (Value::List(a), Value::List(b)) => {
                Rc::ptr_eq(a, b) || {
                    let (a, b) = (a.borrow(), b.borrow());
                    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                Rc::ptr_eq(a, b) || {
                    let (a, b) = (a.borrow(), b.borrow());
                    a.fields.len() == b.fields.len()
                        && a
                            .fields
                            .iter()
                            .all(|(name, value)| b.get(name).is_some_and(|v| value.equals(v)))
                }
            }
            (Value::Variant(a), Value::Variant(b)) => {
                a.name == b.name
                    && a.values.len() == b.values.len()
                    && a.values.iter().zip(&b.values).all(|(x, y)| x.equals(y))
            }
            (Value::Native(a), Value::Native(b)) => {
                Rc::ptr_eq(a, b)
                    || match (&*a.borrow(), &*b.borrow()) {
                        (Native::DateTime(x), Native::DateTime(y)) => x == y,
                        _ => false,
                    }
            }
            _ => self.identical(other),
        }
    }

    /// Identity (`is`): reference values must be the same object in memory.
    pub fn identical(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::List(a), Value::List(b)) => Rc::ptr_eq(a, b),
            (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
            (Value::Variant(a), Value::Variant(b)) => {
                Rc::ptr_eq(a, b) || (a.values.is_empty() && b.values.is_empty() && a.name == b.name)
            }
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (Value::Builtin(a), Value::Builtin(b)) => a == b,
            (Value::Constructor(a), Value::Constructor(b)) => Rc::ptr_eq(a, b),
            (Value::Module(a), Value::Module(b)) => Rc::ptr_eq(a, b),
            (Value::Native(a), Value::Native(b)) => Rc::ptr_eq(a, b),
            (Value::List(_) | Value::Object(_) | Value::Variant(_), _)
            | (_, Value::List(_) | Value::Object(_) | Value::Variant(_)) => false,
            _ => self.equals(other),
        }
    }
}

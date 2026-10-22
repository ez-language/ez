//! Built-in functions, methods and the standard library modules.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::runtime::interpreter::{as_float, some, Exec, Interpreter};
use crate::runtime::value::{DateTime, FileHandle, Native, SumType, Value, VariantValue};
use crate::semantic::builtin_sums::{builtin_sum, stdlib_functions, BUILTIN_SUMS};

mod datetime;
mod file;
mod http;
mod json;
mod math;
mod prelude;
mod regex;
mod string;
mod time;

pub use math::{real_pow, PowFailure};
pub use string::string_method;

thread_local! {
    static SUMS: RefCell<HashMap<&'static str, Rc<SumType>>> = RefCell::new(HashMap::new());
}

/// Runtime description of a built-in sum type (`Option`, `Json`, `FileError`...).
pub fn sum(name: &str) -> Rc<SumType> {
    let definition = builtin_sum(name);
    SUMS.with(|sums| {
        Rc::clone(sums.borrow_mut().entry(definition.name).or_insert_with(|| {
            Rc::new(SumType {
                name: definition.name.to_string(),
                variants: definition
                    .variants
                    .iter()
                    .map(|(variant, fields)| {
                        (variant.to_string(), fields.iter().map(|(f, _)| f.to_string()).collect())
                    })
                    .collect(),
            })
        }))
    })
}

/// A value of a built-in sum type: `variant("FileError", "NotFound", vec![...])`.
fn variant(type_name: &str, name: &str, values: Vec<Value>) -> Value {
    sum(type_name).make(name, values)
}

pub fn none() -> Value {
    variant("Option", "None", vec![])
}

pub fn ok(value: Value) -> Value {
    variant("Result", "Ok", vec![value])
}

pub fn err(value: Value) -> Value {
    variant("Result", "Err", vec![value])
}

/// Members of a standard library module.
pub fn module_members(module: &str) -> Vec<(String, Value)> {
    let mut members: Vec<(String, Value)> = stdlib_functions(module)
        .iter()
        .map(|f| (f.to_string(), Value::Builtin(Rc::from(format!("{}.{}", module, f)))))
        .collect();
    if module == "math" {
        members.push(("pi".to_string(), Value::Float(std::f64::consts::PI)));
        members.push(("e".to_string(), Value::Float(std::f64::consts::E)));
    }
    for definition in BUILTIN_SUMS.iter().filter(|s| s.module == Some(module)) {
        let sum = sum(definition.name);
        for (name, _) in &sum.variants {
            members.push((name.clone(), sum.constructor(name)));
        }
    }
    members
}

fn str_arg<'a>(interpreter: &Interpreter, args: &'a [Value], index: usize) -> Exec<&'a str> {
    match args.get(index) {
        Some(Value::Str(s)) => Ok(s),
        Some(other) => Err(interpreter.error(format!(
            "argument {} must be a string, found {}",
            index + 1,
            other.type_name()
        ))),
        None => Err(interpreter.error(format!("missing argument {}", index + 1))),
    }
}

fn int_arg(interpreter: &Interpreter, args: &[Value], index: usize) -> Exec<i64> {
    match args.get(index) {
        Some(Value::Int(n)) => Ok(*n),
        Some(other) => Err(interpreter.error(format!(
            "argument {} must be an int, found {}",
            index + 1,
            other.type_name()
        ))),
        None => Err(interpreter.error(format!("missing argument {}", index + 1))),
    }
}

fn number_arg(interpreter: &Interpreter, args: &[Value], index: usize) -> Exec<f64> {
    match args.get(index) {
        Some(value @ (Value::Int(_) | Value::Float(_))) => Ok(as_float(value)),
        Some(other) => Err(interpreter.error(format!(
            "argument {} must be a number, found {}",
            index + 1,
            other.type_name()
        ))),
        None => Err(interpreter.error(format!("missing argument {}", index + 1))),
    }
}

/// Converting a float that does not fit in an `int` is a representation limit: it panics.
fn float_to_int(interpreter: &Interpreter, value: f64) -> Exec<Value> {
    if value.is_finite() && value.abs() < 9.2e18 {
        Ok(Value::Int(value as i64))
    } else {
        Err(interpreter.error(format!("integer overflow: {} does not fit in an int", value)))
    }
}

/// Calls the built-in function `name` (`print`, `math.sqrt`...).
pub fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
    match name.split_once('.').map(|(module, _)| module) {
        None => prelude::call(interpreter, name, args),
        Some("time") => time::call(interpreter, name, args),
        Some("file") => file::call(interpreter, name, args),
        Some("datetime") => datetime::call(interpreter, name, args),
        Some("math") => math::call(interpreter, name, args),
        Some("http") => http::call(interpreter, name, args),
        Some("json") => json::call(interpreter, name, args),
        Some("regex") => regex::call(interpreter, name, args),
        Some(_) => Err(interpreter.error(format!("unknown built-in function '{}'", name))),
    }
}

/// Standard library error types whose values have a human-readable message.
pub const ERROR_TYPES: &[&str] = &[
    "ParseError",
    "MathError",
    "FileError",
    "HttpError",
    "JsonError",
    "DateError",
    "RegexError",
];

/// Human-readable text of a standard library error (`NotFound('a.txt')` → `file 'a.txt' not
/// found`), used by `print`, interpolation and `.display()`.
pub fn error_message(error: &VariantValue) -> Option<String> {
    if !ERROR_TYPES.contains(&error.sum.name.as_str()) {
        return None;
    }
    let text = |i: usize| error.values.get(i).map(Value::display).unwrap_or_default();
    let message = match error.name.as_str() {
        "InvalidNumber" => format!("'{}' is not a valid number", text(0)),
        "OutOfRange" => format!("'{}' is out of range", text(0)),
        "Complex" => "the result is not a real number (it is complex)".to_string(),
        "Indeterminate" => "the result is indeterminate".to_string(),
        "Infinite" => "the result is infinite".to_string(),
        "NotFound" => format!("file '{}' not found", text(0)),
        "PermissionDenied" => format!("permission denied for '{}'", text(0)),
        "IsDirectory" => format!("'{}' is a directory", text(0)),
        "InvalidUtf8" => format!("file '{}' is not valid UTF-8 text", text(0)),
        "Io" => format!("cannot access '{}': {}", text(0), text(1)),
        "InvalidUrl" => format!("invalid URL '{}'", text(0)),
        "ConnectionFailed" => format!("cannot connect to '{}': {}", text(0), text(1)),
        "Timeout" => format!("the request to '{}' timed out", text(0)),
        "Tls" => format!("secure connection to '{}' failed: {}", text(0), text(1)),
        "InvalidResponse" => format!("invalid response from '{}': {}", text(0), text(1)),
        "Syntax" => format!("invalid JSON at line {}, column {}: {}", text(1), text(2), text(0)),
        "NumberOutOfRange" => format!(
            "the number {} at line {}, column {} is too large",
            text(0),
            text(1),
            text(2)
        ),
        "PatternMismatch" => format!("'{}' does not match the pattern '{}'", text(0), text(1)),
        "InvalidDate" => format!("'{}' is not a valid date", text(0)),
        "InvalidPattern" => format!("invalid regular expression '{}': {}", text(0), text(1)),
        _ => return None,
    };
    Some(message)
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn native_method(
    interpreter: &mut Interpreter,
    native: &Rc<RefCell<Native>>,
    name: &str,
    args: Vec<Value>,
) -> Exec<Value> {
    let mut native = native.borrow_mut();
    match &mut *native {
        Native::File(handle) => file::method(interpreter, handle, name, args),
        Native::Regex(regex) => regex::method(interpreter, regex, name, args),
        Native::DateTime(_) => Err(interpreter.error(format!("DateTime has no method '{}'", name))),
    }
}

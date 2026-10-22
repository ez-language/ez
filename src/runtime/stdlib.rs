//! Built-in functions, methods and the standard library modules.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::runtime::interpreter::{as_float, some, Exec, Interpreter};
use crate::runtime::value::{DateTime, FileHandle, Native, SumType, Value, VariantValue};
use crate::semantic::builtin_sums::{builtin_sum, stdlib_functions, BUILTIN_SUMS};

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

/// `int('...')`: `Ok(n)`, `Err(InvalidNumber(text))` or `Err(OutOfRange(text))`.
fn parse_int(text: &str) -> Value {
    let trimmed = text.trim();
    match trimmed.parse::<i64>() {
        Ok(n) => ok(Value::Int(n)),
        Err(_) => {
            let digits = trimmed.strip_prefix(['-', '+']).unwrap_or(trimmed);
            let failure = if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                "OutOfRange"
            } else {
                "InvalidNumber"
            };
            err(variant("ParseError", failure, vec![Value::str(text)]))
        }
    }
}

/// `float('...')`: only decimal notation is accepted (no `nan` or `inf`).
fn parse_float(text: &str) -> Value {
    let trimmed = text.trim();
    let well_formed = !trimmed.is_empty()
        && trimmed.chars().any(|c| c.is_ascii_digit())
        && trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E'));
    match trimmed.parse::<f64>() {
        Ok(n) if well_formed && n.is_finite() => ok(Value::Float(n)),
        Ok(_) if well_formed => err(variant("ParseError", "OutOfRange", vec![Value::str(text)])),
        _ => err(variant("ParseError", "InvalidNumber", vec![Value::str(text)])),
    }
}

/// Why a real power cannot be computed.
pub enum PowFailure {
    /// Domain problem: the result is a complex number (`(-8) ** 0.5`).
    Complex,
    /// Domain problem: the result is infinite (`0 ** -1`, a pole). Not an overflow: no larger
    /// type could represent it.
    Infinite,
    /// Representation limit: the result does not fit in a float.
    Overflow,
}

/// `x ** y` on real numbers. `0 ** 0` is `1`, the value of the expression (the indeterminate
/// form only concerns limits).
pub fn real_pow(x: f64, y: f64) -> Result<f64, PowFailure> {
    if x == 0.0 && y < 0.0 {
        return Err(PowFailure::Infinite);
    }
    if x < 0.0 && y.fract() != 0.0 {
        return Err(PowFailure::Complex);
    }
    let result = x.powf(y);
    if result.is_finite() {
        Ok(result)
    } else {
        Err(PowFailure::Overflow)
    }
}

fn math_error(name: &str) -> Value {
    err(variant("MathError", name, vec![]))
}

/// Calls the built-in function `name`.
pub fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
    match name {
        "print" => {
            let text: Vec<String> = args.iter().map(Value::display).collect();
            let line = text.join(" ");
            writeln!(interpreter.out, "{}", line)
                .and_then(|_| interpreter.out.flush())
                .map_err(|e| interpreter.error(format!("cannot write output: {}", e)))?;
            Ok(Value::Void)
        }
        "input" => {
            if let Some(prompt) = args.first() {
                write!(interpreter.out, "{}", prompt.display())
                    .and_then(|_| interpreter.out.flush())
                    .map_err(|e| interpreter.error(format!("cannot write output: {}", e)))?;
            }
            let mut line = String::new();
            let read = interpreter
                .input
                .read_line(&mut line)
                .map_err(|e| interpreter.error(format!("cannot read input: {}", e)))?;
            if read == 0 {
                return Ok(none());
            }
            while line.ends_with('\n') || line.ends_with('\r') {
                line.pop();
            }
            Ok(some(Value::str(&line)))
        }
        "panic" => {
            let message = str_arg(interpreter, &args, 0)?.to_string();
            Err(interpreter.error(message))
        }
        "range" => {
            let (start, end, step) = match args.len() {
                1 => (0, int_arg(interpreter, &args, 0)?, 1),
                2 => (int_arg(interpreter, &args, 0)?, int_arg(interpreter, &args, 1)?, 1),
                _ => (
                    int_arg(interpreter, &args, 0)?,
                    int_arg(interpreter, &args, 1)?,
                    int_arg(interpreter, &args, 2)?,
                ),
            };
            if step == 0 {
                return Err(interpreter.error("range() step cannot be 0"));
            }
            let mut items = Vec::new();
            let mut i = start;
            while (step > 0 && i < end) || (step < 0 && i > end) {
                items.push(Value::Int(i));
                i = match i.checked_add(step) {
                    Some(next) => next,
                    None => break,
                };
            }
            Ok(Value::list(items))
        }
        "int" => match &args[0] {
            Value::Int(n) => Ok(Value::Int(*n)),
            Value::Float(f) => float_to_int(interpreter, f.trunc()),
            Value::Bool(b) => Ok(Value::Int(*b as i64)),
            Value::Str(s) => Ok(parse_int(s)),
            other => Err(interpreter.error(format!("cannot convert {} to int", other.type_name()))),
        },
        "float" => match &args[0] {
            Value::Int(n) => Ok(Value::Float(*n as f64)),
            Value::Float(f) => Ok(Value::Float(*f)),
            Value::Bool(b) => Ok(Value::Float(*b as i64 as f64)),
            Value::Str(s) => Ok(parse_float(s)),
            other => Err(interpreter.error(format!("cannot convert {} to float", other.type_name()))),
        },
        "string" => Ok(Value::str(&args[0].display())),

        "time.sleep" => {
            let seconds = number_arg(interpreter, &args, 0)?;
            if seconds > 0.0 {
                let _ = interpreter.out.flush();
                std::thread::sleep(Duration::from_secs_f64(seconds));
            }
            Ok(Value::Void)
        }
        "time.now" => Ok(Value::Float(unix_now())),
        "time.monotonic" => Ok(Value::Float(interpreter.started.elapsed().as_secs_f64())),

        "file.open" => {
            let path = str_arg(interpreter, &args, 0)?.to_string();
            // `Read`, `Write` or `Append` (the `file.Mode` sum type).
            let mode = match args.get(1) {
                Some(Value::Variant(mode)) => mode.name.clone(),
                _ => return Err(interpreter.error("the mode must be file.Read, file.Write or file.Append")),
            };
            let mut options = std::fs::OpenOptions::new();
            match mode.as_str() {
                "Read" => options.read(true),
                "Write" => options.write(true).create(true).truncate(true),
                _ => options.append(true).create(true),
            };
            match options.open(&path) {
                Ok(file) => Ok(ok(Value::Native(Rc::new(RefCell::new(Native::File(FileHandle {
                    path,
                    mode,
                    file: Some(file),
                })))))),
                Err(error) => Ok(err(file_error(&path, &error))),
            }
        }

        "datetime.now" => Ok(Value::Native(Rc::new(RefCell::new(Native::DateTime(
            datetime_from_timestamp(unix_now().floor() as i64),
        ))))),
        "datetime.today" => {
            let now = datetime_from_timestamp(unix_now().floor() as i64);
            Ok(Value::str(&format!("{:04}-{:02}-{:02}", now.year, now.month, now.day)))
        }
        "datetime.format" => {
            let timestamp = number_arg(interpreter, &args, 0)?;
            let pattern = str_arg(interpreter, &args, 1)?;
            let datetime = datetime_from_timestamp(timestamp.floor() as i64);
            Ok(Value::str(&format_datetime(&datetime, pattern)))
        }
        "datetime.parse" => {
            let text = str_arg(interpreter, &args, 0)?;
            let pattern = str_arg(interpreter, &args, 1)?;
            Ok(match parse_datetime(text, pattern) {
                Ok(timestamp) => ok(Value::Int(timestamp)),
                Err(DateFailure::PatternMismatch) => err(variant(
                    "DateError",
                    "PatternMismatch",
                    vec![Value::str(text), Value::str(pattern)],
                )),
                Err(DateFailure::InvalidDate) => {
                    err(variant("DateError", "InvalidDate", vec![Value::str(text)]))
                }
            })
        }

        "math.abs" => match &args[0] {
            Value::Int(n) => n
                .checked_abs()
                .map(Value::Int)
                .ok_or_else(|| interpreter.error("integer overflow")),
            other => Ok(Value::Float(as_float(other).abs())),
        },
        "math.sqrt" => {
            let x = number_arg(interpreter, &args, 0)?;
            if x < 0.0 {
                return Ok(math_error("Complex"));
            }
            Ok(ok(Value::Float(x.sqrt())))
        }
        "math.pow" => {
            let x = number_arg(interpreter, &args, 0)?;
            let y = number_arg(interpreter, &args, 1)?;
            match real_pow(x, y) {
                Ok(result) => Ok(ok(Value::Float(result))),
                Err(PowFailure::Complex) => Ok(math_error("Complex")),
                Err(PowFailure::Infinite) => Ok(math_error("Infinite")),
                Err(PowFailure::Overflow) => Err(interpreter.error("float overflow")),
            }
        }
        "math.floor" | "math.ceil" | "math.round" => {
            let x = number_arg(interpreter, &args, 0)?;
            let rounded = match name {
                "math.floor" => x.floor(),
                "math.ceil" => x.ceil(),
                _ => x.round(),
            };
            float_to_int(interpreter, rounded)
        }

        "http.get" | "http.delete" => {
            let url = str_arg(interpreter, &args, 0)?.to_string();
            Ok(http_request(&name[5..], &url, None))
        }
        "http.post" | "http.put" => {
            let url = str_arg(interpreter, &args, 0)?.to_string();
            let body = str_arg(interpreter, &args, 1)?.to_string();
            Ok(http_request(&name[5..], &url, Some(body)))
        }

        "json.parse" => {
            let text = str_arg(interpreter, &args, 0)?;
            Ok(match json::parse(text) {
                Ok(value) => ok(value),
                Err(error) => err(error),
            })
        }
        "json.stringify" => {
            let mut out = String::new();
            json::stringify(&args[0], &mut out).map_err(|message| interpreter.error(message))?;
            Ok(Value::str(&out))
        }
        "json.get" => {
            let key = str_arg(interpreter, &args, 1)?;
            Ok(match json_payload(&args[0], "Object") {
                Some(Value::Object(fields)) => match fields.borrow().get(key) {
                    Some(value) => some(value.clone()),
                    None => none(),
                },
                _ => none(),
            })
        }
        "json.asString" => Ok(json_payload(&args[0], "String").map_or_else(none, some)),
        "json.asInt" => Ok(json_payload(&args[0], "Int").map_or_else(none, some)),
        "json.asBool" => Ok(json_payload(&args[0], "Bool").map_or_else(none, some)),
        "json.asFloat" => Ok(match (json_payload(&args[0], "Float"), json_payload(&args[0], "Int")) {
            (Some(value), _) => some(value),
            (_, Some(Value::Int(n))) => some(Value::Float(n as f64)),
            _ => none(),
        }),

        "regex.compile" => {
            let pattern = str_arg(interpreter, &args, 0)?;
            Ok(match regex::Regex::new(pattern) {
                Ok(regex) => ok(Value::Native(Rc::new(RefCell::new(Native::Regex(regex))))),
                Err(error) => err(variant(
                    "RegexError",
                    "InvalidPattern",
                    vec![Value::str(pattern), Value::str(&regex_error_message(&error))],
                )),
            })
        }

        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
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

/// The payload of a `Json` value if it is the variant `name` (`String('a')` → `'a'`).
fn json_payload(value: &Value, name: &str) -> Option<Value> {
    match value {
        Value::Variant(v) if v.name == name => v.values.first().cloned(),
        _ => None,
    }
}

fn file_error(path: &str, error: &std::io::Error) -> Value {
    use std::io::ErrorKind;
    let path_value = Value::str(path);
    match error.kind() {
        ErrorKind::NotFound => variant("FileError", "NotFound", vec![path_value]),
        ErrorKind::PermissionDenied => variant("FileError", "PermissionDenied", vec![path_value]),
        ErrorKind::IsADirectory => variant("FileError", "IsDirectory", vec![path_value]),
        ErrorKind::InvalidData => variant("FileError", "InvalidUtf8", vec![path_value]),
        _ => variant("FileError", "Io", vec![path_value, Value::str(&error.to_string())]),
    }
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ----------------------------------------------------------------------
// Methods
// ----------------------------------------------------------------------

/// Python-like `istitle`: every word starts with an uppercase letter followed by lowercase ones.
fn is_title(s: &str) -> bool {
    let mut cased = false;
    let mut previous_cased = false;
    for c in s.chars() {
        if c.is_uppercase() {
            if previous_cased {
                return false;
            }
            previous_cased = true;
            cased = true;
        } else if c.is_lowercase() {
            if !previous_cased {
                return false;
            }
            previous_cased = true;
            cased = true;
        } else {
            previous_cased = false;
        }
    }
    cased
}

pub fn string_method(interpreter: &mut Interpreter, s: &str, name: &str, args: Vec<Value>) -> Exec<Value> {
    let all = |predicate: fn(char) -> bool| !s.is_empty() && s.chars().all(predicate);
    let has_cased = s.chars().any(|c| c.is_uppercase() || c.is_lowercase());
    Ok(match name {
        "length" => Value::Int(s.chars().count() as i64),
        "trim" => Value::str(s.trim()),
        "upper" => Value::str(&s.to_uppercase()),
        "lower" => Value::str(&s.to_lowercase()),
        "isSpace" => Value::Bool(all(char::is_whitespace)),
        "isNumeric" => Value::Bool(all(char::is_numeric)),
        "isAlpha" => Value::Bool(all(char::is_alphabetic)),
        "isAlphaNumeric" => Value::Bool(all(char::is_alphanumeric)),
        "isUpper" => Value::Bool(has_cased && !s.chars().any(char::is_lowercase)),
        "isLower" => Value::Bool(has_cased && !s.chars().any(char::is_uppercase)),
        "isTitle" => Value::Bool(is_title(s)),
        "split" => {
            let separator = str_arg(interpreter, &args, 0)?;
            if separator.is_empty() {
                return Err(interpreter.error("split() separator cannot be empty"));
            }
            Value::list(s.split(separator).map(Value::str).collect())
        }
        "replace" => {
            let old = str_arg(interpreter, &args, 0)?;
            let new = str_arg(interpreter, &args, 1)?;
            if old.is_empty() {
                return Err(interpreter.error("replace() needs a non-empty text to replace"));
            }
            Value::str(&s.replace(old, new))
        }
        other => return Err(interpreter.error(format!("strings have no method '{}'", other))),
    })
}

pub fn native_method(
    interpreter: &mut Interpreter,
    native: &Rc<RefCell<Native>>,
    name: &str,
    args: Vec<Value>,
) -> Exec<Value> {
    let mut native = native.borrow_mut();
    let handle = match &mut *native {
        Native::File(handle) => handle,
        Native::Regex(regex) => return regex_method(interpreter, regex, name, args),
        Native::DateTime(_) => {
            return Err(interpreter.error(format!("DateTime has no method '{}'", name)))
        }
    };
    let path = handle.path.clone();
    let mode = handle.mode.clone();
    if name == "close" {
        handle.file = None;
        return Ok(Value::Void);
    }
    let file = match handle.file.as_mut() {
        Some(file) => file,
        None => return Err(interpreter.error(format!("file '{}' is closed", path))),
    };
    match name {
        "read" => {
            if mode != "Read" {
                return Err(interpreter.error(format!("file '{}' was not opened for reading (Read)", path)));
            }
            let mut content = String::new();
            Ok(match file.read_to_string(&mut content) {
                Ok(_) => ok(Value::str(&content)),
                Err(error) => err(file_error(&path, &error)),
            })
        }
        "write" => {
            if mode == "Read" {
                return Err(interpreter.error(format!(
                    "file '{}' was opened for reading; use Write or Append to write",
                    path
                )));
            }
            let text = str_arg(interpreter, &args, 0)?;
            Ok(match file.write_all(text.as_bytes()) {
                Ok(()) => ok(Value::Void),
                Err(error) => err(file_error(&path, &error)),
            })
        }
        other => Err(interpreter.error(format!("File has no method '{}'", other))),
    }
}

// ----------------------------------------------------------------------
// datetime (UTC)
// ----------------------------------------------------------------------

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

pub fn datetime_from_timestamp(timestamp: i64) -> DateTime {
    let days = timestamp.div_euclid(86_400);
    let seconds = timestamp.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    DateTime {
        year,
        month,
        day,
        hour: seconds / 3600,
        minute: seconds % 3600 / 60,
        second: seconds % 60,
    }
}

#[derive(Clone, Copy)]
enum DateToken {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

/// Splits a pattern into tokens (`yyyy`, `mm`, `dd`, `hh`, `min`, `ss`, any case) and literal text.
fn date_tokens(pattern: &str) -> Vec<Result<DateToken, char>> {
    let chars: Vec<char> = pattern.chars().collect();
    let lower: Vec<char> = pattern.to_lowercase().chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let starts = |i: usize, word: &str| -> bool {
        word.chars().enumerate().all(|(k, c)| lower.get(i + k) == Some(&c))
    };
    while i < chars.len() {
        let (token, len) = if starts(i, "yyyy") {
            (Ok(DateToken::Year), 4)
        } else if starts(i, "min") {
            (Ok(DateToken::Minute), 3)
        } else if starts(i, "mm") {
            (Ok(DateToken::Month), 2)
        } else if starts(i, "dd") {
            (Ok(DateToken::Day), 2)
        } else if starts(i, "hh") {
            (Ok(DateToken::Hour), 2)
        } else if starts(i, "ss") {
            (Ok(DateToken::Second), 2)
        } else {
            (Err(chars[i]), 1)
        };
        tokens.push(token);
        i += len;
    }
    tokens
}

pub fn format_datetime(datetime: &DateTime, pattern: &str) -> String {
    let mut out = String::new();
    for token in date_tokens(pattern) {
        match token {
            Ok(DateToken::Year) => out.push_str(&format!("{:04}", datetime.year)),
            Ok(DateToken::Month) => out.push_str(&format!("{:02}", datetime.month)),
            Ok(DateToken::Day) => out.push_str(&format!("{:02}", datetime.day)),
            Ok(DateToken::Hour) => out.push_str(&format!("{:02}", datetime.hour)),
            Ok(DateToken::Minute) => out.push_str(&format!("{:02}", datetime.minute)),
            Ok(DateToken::Second) => out.push_str(&format!("{:02}", datetime.second)),
            Err(c) => out.push(c),
        }
    }
    out
}

#[derive(Debug)]
pub enum DateFailure {
    PatternMismatch,
    InvalidDate,
}

pub fn parse_datetime(text: &str, pattern: &str) -> Result<i64, DateFailure> {
    let chars: Vec<char> = text.chars().collect();
    let mut position = 0;
    let mut datetime = DateTime {
        year: 1970,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
    };
    let mismatch = || DateFailure::PatternMismatch;
    for token in date_tokens(pattern) {
        match token {
            Err(c) => {
                if chars.get(position) != Some(&c) {
                    return Err(mismatch());
                }
                position += 1;
            }
            Ok(token) => {
                let width = if matches!(token, DateToken::Year) { 4 } else { 2 };
                let digits: String = chars.iter().skip(position).take(width).collect();
                if digits.len() != width || !digits.chars().all(|c| c.is_ascii_digit()) {
                    return Err(mismatch());
                }
                position += width;
                let n: i64 = digits.parse().map_err(|_| mismatch())?;
                match token {
                    DateToken::Year => datetime.year = n,
                    DateToken::Month => datetime.month = n,
                    DateToken::Day => datetime.day = n,
                    DateToken::Hour => datetime.hour = n,
                    DateToken::Minute => datetime.minute = n,
                    DateToken::Second => datetime.second = n,
                }
            }
        }
    }
    if position != chars.len() {
        return Err(mismatch());
    }
    let valid = (1..=12).contains(&datetime.month)
        && datetime.day >= 1
        && datetime.day <= days_in_month(datetime.year, datetime.month)
        && datetime.hour < 24
        && datetime.minute < 60
        && datetime.second < 60;
    if !valid {
        return Err(DateFailure::InvalidDate);
    }
    Ok(days_from_civil(datetime.year, datetime.month, datetime.day) * 86_400
        + datetime.hour * 3600
        + datetime.minute * 60
        + datetime.second)
}

// ----------------------------------------------------------------------
// http
// ----------------------------------------------------------------------

fn http_error(url: &str, error: &ureq::Transport) -> Value {
    use ureq::ErrorKind;
    let message = error.to_string();
    let lower = message.to_lowercase();
    let url_value = Value::str(url);
    let with_message = |name: &str| variant("HttpError", name, vec![Value::str(url), Value::str(&message)]);
    match error.kind() {
        ErrorKind::InvalidUrl | ErrorKind::UnknownScheme => {
            variant("HttpError", "InvalidUrl", vec![url_value])
        }
        _ if lower.contains("timed out") || lower.contains("timeout") => {
            variant("HttpError", "Timeout", vec![url_value])
        }
        _ if lower.contains("tls") || lower.contains("certificate") => with_message("Tls"),
        ErrorKind::TooManyRedirects | ErrorKind::BadStatus | ErrorKind::BadHeader => {
            with_message("InvalidResponse")
        }
        _ => with_message("ConnectionFailed"),
    }
}

/// Sends a request. 4xx and 5xx statuses are still `Ok(Response)`: only transport failures are
/// errors.
fn http_request(method: &str, url: &str, body: Option<String>) -> Value {
    let request = ureq::request(&method.to_uppercase(), url);
    let response = match body {
        Some(body) => {
            let trimmed = body.trim_start();
            let content_type = if trimmed.starts_with('{') || trimmed.starts_with('[') {
                "application/json"
            } else {
                "text/plain; charset=utf-8"
            };
            request.set("Content-Type", content_type).send_string(&body)
        }
        None => request.call(),
    };
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(ureq::Error::Transport(error)) => return err(http_error(url, &error)),
    };
    let status = response.status() as i64;
    match response.into_string() {
        Ok(body) => ok(Value::object(vec![
            ("status".to_string(), Value::Int(status)),
            ("body".to_string(), Value::str(&body)),
        ])),
        Err(error) => err(variant(
            "HttpError",
            "InvalidResponse",
            vec![Value::str(url), Value::str(&error.to_string())],
        )),
    }
}

// ----------------------------------------------------------------------
// regex
// ----------------------------------------------------------------------

/// The last line of the `regex` crate's message (`unclosed group`), without the drawing of the
/// pattern above it.
fn regex_error_message(error: &regex::Error) -> String {
    let text = error.to_string();
    let last = text.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("");
    last.trim().trim_start_matches("error: ").to_string()
}

/// Methods of a compiled `Regex`.
fn regex_method(interpreter: &mut Interpreter, regex: &regex::Regex, name: &str, args: Vec<Value>) -> Exec<Value> {
    let input = str_arg(interpreter, &args, 0)?;
    match name {
        "test" => Ok(Value::Bool(regex.is_match(input))),
        "find" => Ok(match regex.find(input) {
            Some(found) => some(match_object(input, &found)),
            None => none(),
        }),
        "findAll" => Ok(Value::list(
            regex.find_iter(input).map(|m| match_object(input, &m)).collect(),
        )),
        "replace" | "replaceAll" => {
            let replacement = str_arg(interpreter, &args, 1)?;
            Ok(Value::str(&if name == "replace" {
                regex.replace(input, replacement)
            } else {
                regex.replace_all(input, replacement)
            }))
        }
        "split" => Ok(Value::list(regex.split(input).map(Value::str).collect())),
        other => Err(interpreter.error(format!("Regex has no method '{}'", other))),
    }
}

/// `Match { value, start, end }`, with positions counted in characters.
fn match_object(input: &str, found: &regex::Match) -> Value {
    let start = input[..found.start()].chars().count() as i64;
    let end = start + found.as_str().chars().count() as i64;
    Value::object(vec![
        ("value".to_string(), Value::str(found.as_str())),
        ("start".to_string(), Value::Int(start)),
        ("end".to_string(), Value::Int(end)),
    ])
}

// ----------------------------------------------------------------------
// json
// ----------------------------------------------------------------------

mod json {
    use super::{variant, Value};
    use std::fmt::Write as _;

    fn json(name: &str, values: Vec<Value>) -> Value {
        variant("Json", name, values)
    }

    /// Parses JSON into a `Json` value; on failure, returns a `JsonError` value.
    pub fn parse(text: &str) -> Result<Value, Value> {
        let mut parser = JsonParser {
            chars: text.chars().collect(),
            position: 0,
        };
        let result = parser.value().and_then(|value| {
            parser.skip_whitespace();
            if parser.position == parser.chars.len() {
                Ok(value)
            } else {
                Err(parser.syntax("unexpected character after the JSON value"))
            }
        });
        result.map_err(|failure| failure.into_value(&parser.chars))
    }

    enum Failure {
        Syntax(String, usize),
        NumberOutOfRange(String, usize),
    }

    impl Failure {
        /// `JsonError` value with a 1-based line and column.
        fn into_value(self, chars: &[char]) -> Value {
            let position = match &self {
                Failure::Syntax(_, p) | Failure::NumberOutOfRange(_, p) => *p,
            };
            let before = &chars[..position.min(chars.len())];
            let line = before.iter().filter(|c| **c == '\n').count() + 1;
            let column = before.iter().rev().take_while(|c| **c != '\n').count() + 1;
            let (name, text) = match self {
                Failure::Syntax(message, _) => ("Syntax", message),
                Failure::NumberOutOfRange(number, _) => ("NumberOutOfRange", number),
            };
            variant(
                "JsonError",
                name,
                vec![Value::str(&text), Value::Int(line as i64), Value::Int(column as i64)],
            )
        }
    }

    struct JsonParser {
        chars: Vec<char>,
        position: usize,
    }

    impl JsonParser {
        fn syntax(&self, message: &str) -> Failure {
            Failure::Syntax(message.to_string(), self.position)
        }

        fn skip_whitespace(&mut self) {
            while matches!(self.chars.get(self.position), Some(c) if c.is_whitespace()) {
                self.position += 1;
            }
        }

        fn expect(&mut self, c: char) -> Result<(), Failure> {
            self.skip_whitespace();
            if self.chars.get(self.position) == Some(&c) {
                self.position += 1;
                Ok(())
            } else {
                Err(self.syntax(&format!("expected '{}'", c)))
            }
        }

        fn literal(&mut self, word: &str, value: Value) -> Result<Value, Failure> {
            for c in word.chars() {
                if self.chars.get(self.position) != Some(&c) {
                    return Err(self.syntax("invalid literal"));
                }
                self.position += 1;
            }
            Ok(value)
        }

        fn value(&mut self) -> Result<Value, Failure> {
            self.skip_whitespace();
            match self.chars.get(self.position) {
                Some('{') => {
                    self.position += 1;
                    let mut fields: Vec<(String, Value)> = Vec::new();
                    self.skip_whitespace();
                    if self.chars.get(self.position) == Some(&'}') {
                        self.position += 1;
                        return Ok(json("Object", vec![Value::object(fields)]));
                    }
                    loop {
                        self.skip_whitespace();
                        let key = self.string()?;
                        self.expect(':')?;
                        let value = self.value()?;
                        fields.retain(|(k, _)| *k != key);
                        fields.push((key, value));
                        self.skip_whitespace();
                        match self.chars.get(self.position) {
                            Some(',') => self.position += 1,
                            Some('}') => {
                                self.position += 1;
                                return Ok(json("Object", vec![Value::object(fields)]));
                            }
                            _ => return Err(self.syntax("expected ',' or '}'")),
                        }
                    }
                }
                Some('[') => {
                    self.position += 1;
                    let mut items = Vec::new();
                    self.skip_whitespace();
                    if self.chars.get(self.position) == Some(&']') {
                        self.position += 1;
                        return Ok(json("Array", vec![Value::list(items)]));
                    }
                    loop {
                        items.push(self.value()?);
                        self.skip_whitespace();
                        match self.chars.get(self.position) {
                            Some(',') => self.position += 1,
                            Some(']') => {
                                self.position += 1;
                                return Ok(json("Array", vec![Value::list(items)]));
                            }
                            _ => return Err(self.syntax("expected ',' or ']'")),
                        }
                    }
                }
                Some('"') => Ok(json("String", vec![Value::str(&self.string()?)])),
                Some('t') => self.literal("true", json("Bool", vec![Value::Bool(true)])),
                Some('f') => self.literal("false", json("Bool", vec![Value::Bool(false)])),
                Some('n') => self.literal("null", json("Null", vec![])),
                Some(c) if *c == '-' || c.is_ascii_digit() => self.number(),
                Some(c) => Err(self.syntax(&format!("unexpected character '{}'", c))),
                None => Err(self.syntax("unexpected end of input")),
            }
        }

        /// Numbers without `.`, `e` or `E` are `Int`; the others are `Float`.
        fn number(&mut self) -> Result<Value, Failure> {
            let start = self.position;
            while matches!(
                self.chars.get(self.position),
                Some(c) if c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')
            ) {
                self.position += 1;
            }
            let text: String = self.chars[start..self.position].iter().collect();
            let out_of_range = || Failure::NumberOutOfRange(text.clone(), start);
            let invalid = || Failure::Syntax(format!("invalid number '{}'", text), start);
            if !text.contains(['.', 'e', 'E']) {
                let digits = text.strip_prefix('-').unwrap_or(&text);
                if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
                    return Err(invalid());
                }
                return match text.parse::<i64>() {
                    Ok(n) => Ok(json("Int", vec![Value::Int(n)])),
                    Err(_) => Err(out_of_range()),
                };
            }
            match text.parse::<f64>() {
                Ok(n) if n.is_finite() => Ok(json("Float", vec![Value::Float(n)])),
                Ok(_) => Err(out_of_range()),
                Err(_) => Err(invalid()),
            }
        }

        fn string(&mut self) -> Result<String, Failure> {
            if self.chars.get(self.position) != Some(&'"') {
                return Err(self.syntax("expected a string"));
            }
            self.position += 1;
            let mut out = String::new();
            loop {
                match self.chars.get(self.position).copied() {
                    Some('"') => {
                        self.position += 1;
                        return Ok(out);
                    }
                    Some('\\') => {
                        self.position += 1;
                        let escaped = self.chars.get(self.position).copied();
                        self.position += 1;
                        match escaped {
                            Some('n') => out.push('\n'),
                            Some('t') => out.push('\t'),
                            Some('r') => out.push('\r'),
                            Some('b') => out.push('\u{8}'),
                            Some('f') => out.push('\u{c}'),
                            Some('u') => {
                                let code = self.hex4()?;
                                let c = if (0xD800..0xDC00).contains(&code)
                                    && self.chars.get(self.position) == Some(&'\\')
                                    && self.chars.get(self.position + 1) == Some(&'u')
                                {
                                    self.position += 2;
                                    let low = self.hex4()?;
                                    char::from_u32(
                                        0x10000 + ((code - 0xD800) << 10) + low.wrapping_sub(0xDC00),
                                    )
                                } else {
                                    char::from_u32(code)
                                };
                                out.push(c.unwrap_or('\u{FFFD}'));
                            }
                            Some(other) => out.push(other),
                            None => return Err(self.syntax("unterminated string")),
                        }
                    }
                    Some(c) => {
                        out.push(c);
                        self.position += 1;
                    }
                    None => return Err(self.syntax("unterminated string")),
                }
            }
        }

        fn hex4(&mut self) -> Result<u32, Failure> {
            let text: String = self.chars.iter().skip(self.position).take(4).collect();
            let code = u32::from_str_radix(&text, 16)
                .map_err(|_| self.syntax(&format!("invalid escape '\\u{}'", text)))?;
            self.position += 4;
            Ok(code)
        }
    }

    fn write_string(s: &str, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    let _ = write!(out, "\\u{:04x}", c as u32);
                }
                c => out.push(c),
            }
        }
        out.push('"');
    }

    /// Writes a `Json` value. Floats always keep a `.` or an exponent, so that parsing the text
    /// back gives a `Float` again.
    pub fn stringify(value: &Value, out: &mut String) -> Result<(), String> {
        let variant = match value {
            Value::Variant(variant) if variant.sum.name == "Json" => variant,
            other => return Err(format!("expected a Json value, found {}", other.type_name())),
        };
        match (variant.name.as_str(), variant.values.first()) {
            ("Null", _) => out.push_str("null"),
            ("Bool", Some(Value::Bool(b))) => {
                let _ = write!(out, "{}", b);
            }
            ("Int", Some(Value::Int(n))) => {
                let _ = write!(out, "{}", n);
            }
            ("Float", Some(Value::Float(f))) => {
                let text = if f.abs() >= 1e16 {
                    format!("{:e}", f)
                } else {
                    format!("{}", f)
                };
                out.push_str(&text);
                if !text.contains(['.', 'e', 'E']) {
                    out.push_str(".0");
                }
            }
            ("Float", Some(Value::Int(n))) => {
                let _ = write!(out, "{}.0", n);
            }
            ("String", Some(Value::Str(s))) => write_string(s, out),
            ("Array", Some(Value::List(items))) => {
                out.push('[');
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    stringify(item, out)?;
                }
                out.push(']');
            }
            ("Object", Some(Value::Object(object))) => {
                out.push('{');
                for (i, (key, value)) in object.borrow().fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(key, out);
                    out.push(':');
                    stringify(value, out)?;
                }
                out.push('}');
            }
            (name, _) => return Err(format!("invalid Json value '{}'", name)),
        }
        Ok(())
    }
}

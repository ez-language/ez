//! Built-in functions, methods and the standard library modules.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::runtime::interpreter::{as_float, some, Exec, Interpreter};
use crate::runtime::value::{DateTime, FileHandle, Native, Value, VariantValue};

pub fn none() -> Value {
    Value::Variant(Rc::new(VariantValue {
        type_name: "Option".to_string(),
        name: "None".to_string(),
        field_names: Rc::new(vec![]),
        values: vec![],
    }))
}

fn result(ok: bool, value: Value) -> Value {
    let (name, field) = if ok { ("Ok", "value") } else { ("Err", "error") };
    Value::Variant(Rc::new(VariantValue {
        type_name: "Result".to_string(),
        name: name.to_string(),
        field_names: Rc::new(vec![field.to_string()]),
        values: vec![value],
    }))
}

/// Members of a standard library module.
pub fn module_members(module: &str) -> Vec<(String, Value)> {
    let functions: &[&str] = match module {
        "time" => &["sleep", "now", "monotonic"],
        "file" => &["open"],
        "datetime" => &["now", "today", "format", "parse"],
        "math" => &["abs", "sqrt", "pow", "floor", "ceil", "round"],
        "http" => &["get", "post", "put", "delete"],
        "json" => &["parse", "stringify"],
        "regex" => &["match", "search", "findAll", "replace"],
        _ => &[],
    };
    let mut members: Vec<(String, Value)> = functions
        .iter()
        .map(|f| (f.to_string(), Value::Builtin(Rc::from(format!("{}.{}", module, f)))))
        .collect();
    if module == "math" {
        members.push(("pi".to_string(), Value::Float(std::f64::consts::PI)));
        members.push(("e".to_string(), Value::Float(std::f64::consts::E)));
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

fn float_to_int(interpreter: &Interpreter, value: f64) -> Exec<Value> {
    if value.is_finite() && value.abs() < 9.2e18 {
        Ok(Value::Int(value as i64))
    } else {
        Err(interpreter.error(format!("{} cannot be converted to int", value)))
    }
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
            Value::Str(s) => Ok(match s.trim().parse::<i64>() {
                Ok(n) => result(true, Value::Int(n)),
                Err(_) => result(false, Value::str("not a valid int")),
            }),
            other => Err(interpreter.error(format!("cannot convert {} to int", other.type_name()))),
        },
        "float" => match &args[0] {
            Value::Int(n) => Ok(Value::Float(*n as f64)),
            Value::Float(f) => Ok(Value::Float(*f)),
            Value::Bool(b) => Ok(Value::Float(*b as i64 as f64)),
            Value::Str(s) => Ok(match s.trim().parse::<f64>() {
                Ok(n) if !s.trim().is_empty() => result(true, Value::Float(n)),
                _ => result(false, Value::str("not a valid float")),
            }),
            other => Err(interpreter.error(format!("cannot convert {} to float", other.type_name()))),
        },
        "string" => Ok(Value::str(&args[0].display())),

        "time.sleep" => {
            let seconds = number_arg(interpreter, &args, 0)?;
            if seconds > 0.0 && seconds.is_finite() {
                let _ = interpreter.out.flush();
                std::thread::sleep(Duration::from_secs_f64(seconds));
            }
            Ok(Value::Void)
        }
        "time.now" => Ok(Value::Float(unix_now())),
        "time.monotonic" => Ok(Value::Float(interpreter.started.elapsed().as_secs_f64())),

        "file.open" => {
            let path = str_arg(interpreter, &args, 0)?.to_string();
            let mode = str_arg(interpreter, &args, 1)?.to_string();
            let mut options = std::fs::OpenOptions::new();
            match mode.as_str() {
                "r" => options.read(true),
                "w" => options.write(true).create(true).truncate(true),
                "a" => options.append(true).create(true),
                other => {
                    return Err(interpreter.error(format!(
                        "invalid file mode '{}' (use 'r', 'w' or 'a')",
                        other
                    )))
                }
            };
            let file = options
                .open(&path)
                .map_err(|e| interpreter.error(format!("cannot open '{}': {}", path, e)))?;
            Ok(Value::Native(Rc::new(RefCell::new(Native::File(FileHandle {
                path,
                mode,
                file: Some(file),
            })))))
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
            match parse_datetime(text, pattern) {
                Ok(timestamp) => Ok(Value::Int(timestamp)),
                Err(message) => Err(interpreter.error(message)),
            }
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
                return Err(interpreter.error("square root of a negative number"));
            }
            Ok(Value::Float(x.sqrt()))
        }
        "math.pow" => {
            let x = number_arg(interpreter, &args, 0)?;
            let y = number_arg(interpreter, &args, 1)?;
            Ok(Value::Float(x.powf(y)))
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
            http_request(interpreter, &name[5..], &url, None)
        }
        "http.post" | "http.put" => {
            let url = str_arg(interpreter, &args, 0)?.to_string();
            let body = str_arg(interpreter, &args, 1)?.to_string();
            http_request(interpreter, &name[5..], &url, Some(body))
        }

        "json.parse" => {
            let text = str_arg(interpreter, &args, 0)?;
            json::parse(text).map_err(|message| interpreter.error(format!("invalid JSON: {}", message)))
        }
        "json.stringify" => {
            let mut out = String::new();
            json::stringify(&args[0], &mut out).map_err(|message| interpreter.error(message))?;
            Ok(Value::str(&out))
        }

        "regex.match" => {
            let pattern = str_arg(interpreter, &args, 0)?;
            let input = str_arg(interpreter, &args, 1)?;
            let regex = compile_regex(interpreter, &format!("^(?:{})$", pattern))?;
            Ok(Value::Bool(regex.is_match(input)))
        }
        "regex.search" => {
            let pattern = str_arg(interpreter, &args, 0)?;
            let input = str_arg(interpreter, &args, 1)?;
            let regex = compile_regex(interpreter, pattern)?;
            Ok(match regex.find(input) {
                Some(found) => some(match_object(input, &found)),
                None => none(),
            })
        }
        "regex.findAll" => {
            let pattern = str_arg(interpreter, &args, 0)?;
            let input = str_arg(interpreter, &args, 1)?;
            let regex = compile_regex(interpreter, pattern)?;
            Ok(Value::list(
                regex.find_iter(input).map(|m| match_object(input, &m)).collect(),
            ))
        }
        "regex.replace" => {
            let pattern = str_arg(interpreter, &args, 0)?;
            let replacement = str_arg(interpreter, &args, 1)?;
            let input = str_arg(interpreter, &args, 2)?;
            let regex = compile_regex(interpreter, pattern)?;
            Ok(Value::str(&regex.replace_all(input, replacement)))
        }

        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
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
            if mode != "r" {
                return Err(interpreter.error(format!("file '{}' was not opened for reading ('r')", path)));
            }
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| interpreter.error(format!("cannot read '{}': {}", path, e)))?;
            Ok(Value::str(&content))
        }
        "write" => {
            if mode == "r" {
                return Err(interpreter.error(format!(
                    "file '{}' was opened for reading; use 'w' or 'a' to write",
                    path
                )));
            }
            let text = str_arg(interpreter, &args, 0)?;
            file.write_all(text.as_bytes())
                .map_err(|e| interpreter.error(format!("cannot write '{}': {}", path, e)))?;
            Ok(Value::Void)
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

pub fn parse_datetime(text: &str, pattern: &str) -> Result<i64, String> {
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
    let mismatch = || format!("'{}' does not match the pattern '{}'", text, pattern);
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
        return Err(format!("invalid date '{}'", text));
    }
    Ok(days_from_civil(datetime.year, datetime.month, datetime.day) * 86_400
        + datetime.hour * 3600
        + datetime.minute * 60
        + datetime.second)
}

// ----------------------------------------------------------------------
// http
// ----------------------------------------------------------------------

fn http_request(interpreter: &mut Interpreter, method: &str, url: &str, body: Option<String>) -> Exec<Value> {
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
        Err(error) => return Err(interpreter.error(format!("HTTP request to '{}' failed: {}", url, error))),
    };
    let status = response.status() as i64;
    let body = response
        .into_string()
        .map_err(|e| interpreter.error(format!("cannot read the response of '{}': {}", url, e)))?;
    Ok(Value::object(vec![
        ("status".to_string(), Value::Int(status)),
        ("body".to_string(), Value::str(&body)),
    ]))
}

// ----------------------------------------------------------------------
// regex
// ----------------------------------------------------------------------

fn compile_regex(interpreter: &mut Interpreter, pattern: &str) -> Exec<regex::Regex> {
    if let Some(regex) = interpreter.regex_cache.get(pattern) {
        return Ok(regex.clone());
    }
    let regex = regex::Regex::new(pattern)
        .map_err(|e| interpreter.error(format!("invalid regular expression: {}", e)))?;
    interpreter.regex_cache.insert(pattern.to_string(), regex.clone());
    Ok(regex)
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
    use super::{none, Value};
    use crate::runtime::value::Native;
    use std::fmt::Write as _;

    pub fn parse(text: &str) -> Result<Value, String> {
        let mut parser = JsonParser {
            chars: text.chars().collect(),
            position: 0,
        };
        let value = parser.value()?;
        parser.skip_whitespace();
        if parser.position != parser.chars.len() {
            return Err(format!("unexpected character at position {}", parser.position));
        }
        Ok(value)
    }

    struct JsonParser {
        chars: Vec<char>,
        position: usize,
    }

    impl JsonParser {
        fn skip_whitespace(&mut self) {
            while matches!(self.chars.get(self.position), Some(c) if c.is_whitespace()) {
                self.position += 1;
            }
        }

        fn expect(&mut self, c: char) -> Result<(), String> {
            self.skip_whitespace();
            if self.chars.get(self.position) == Some(&c) {
                self.position += 1;
                Ok(())
            } else {
                Err(format!("expected '{}' at position {}", c, self.position))
            }
        }

        fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
            for c in word.chars() {
                if self.chars.get(self.position) != Some(&c) {
                    return Err(format!("invalid literal at position {}", self.position));
                }
                self.position += 1;
            }
            Ok(value)
        }

        fn value(&mut self) -> Result<Value, String> {
            self.skip_whitespace();
            match self.chars.get(self.position) {
                Some('{') => {
                    self.position += 1;
                    let mut fields = Vec::new();
                    self.skip_whitespace();
                    if self.chars.get(self.position) == Some(&'}') {
                        self.position += 1;
                        return Ok(Value::object(fields));
                    }
                    loop {
                        self.skip_whitespace();
                        let key = self.string()?;
                        self.expect(':')?;
                        let value = self.value()?;
                        fields.retain(|(k, _): &(String, Value)| *k != key);
                        fields.push((key, value));
                        self.skip_whitespace();
                        match self.chars.get(self.position) {
                            Some(',') => self.position += 1,
                            Some('}') => {
                                self.position += 1;
                                return Ok(Value::object(fields));
                            }
                            _ => return Err(format!("expected ',' or '}}' at position {}", self.position)),
                        }
                    }
                }
                Some('[') => {
                    self.position += 1;
                    let mut items = Vec::new();
                    self.skip_whitespace();
                    if self.chars.get(self.position) == Some(&']') {
                        self.position += 1;
                        return Ok(Value::list(items));
                    }
                    loop {
                        items.push(self.value()?);
                        self.skip_whitespace();
                        match self.chars.get(self.position) {
                            Some(',') => self.position += 1,
                            Some(']') => {
                                self.position += 1;
                                return Ok(Value::list(items));
                            }
                            _ => return Err(format!("expected ',' or ']' at position {}", self.position)),
                        }
                    }
                }
                Some('"') => Ok(Value::str(&self.string()?)),
                Some('t') => self.literal("true", Value::Bool(true)),
                Some('f') => self.literal("false", Value::Bool(false)),
                Some('n') => self.literal("null", none()),
                Some(c) if *c == '-' || c.is_ascii_digit() => self.number(),
                Some(c) => Err(format!("unexpected character '{}' at position {}", c, self.position)),
                None => Err("unexpected end of input".to_string()),
            }
        }

        fn number(&mut self) -> Result<Value, String> {
            let start = self.position;
            while matches!(
                self.chars.get(self.position),
                Some(c) if c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')
            ) {
                self.position += 1;
            }
            let text: String = self.chars[start..self.position].iter().collect();
            if !text.contains(['.', 'e', 'E']) {
                if let Ok(n) = text.parse::<i64>() {
                    return Ok(Value::Int(n));
                }
            }
            text.parse::<f64>()
                .map(Value::Float)
                .map_err(|_| format!("invalid number '{}'", text))
        }

        fn string(&mut self) -> Result<String, String> {
            if self.chars.get(self.position) != Some(&'"') {
                return Err(format!("expected a string at position {}", self.position));
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
                                    char::from_u32(0x10000 + ((code - 0xD800) << 10) + (low.wrapping_sub(0xDC00)))
                                } else {
                                    char::from_u32(code)
                                };
                                out.push(c.unwrap_or('\u{FFFD}'));
                            }
                            Some(other) => out.push(other),
                            None => return Err("unterminated string".to_string()),
                        }
                    }
                    Some(c) => {
                        out.push(c);
                        self.position += 1;
                    }
                    None => return Err("unterminated string".to_string()),
                }
            }
        }

        fn hex4(&mut self) -> Result<u32, String> {
            let text: String = self.chars.iter().skip(self.position).take(4).collect();
            self.position += 4;
            u32::from_str_radix(&text, 16).map_err(|_| format!("invalid escape '\\u{}'", text))
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

    pub fn stringify(value: &Value, out: &mut String) -> Result<(), String> {
        match value {
            Value::Int(n) => {
                let _ = write!(out, "{}", n);
            }
            Value::Float(f) => {
                if !f.is_finite() {
                    return Err(format!("{} cannot be represented in JSON", f));
                }
                let _ = write!(out, "{}", f);
            }
            Value::Str(s) => write_string(s, out),
            Value::Bool(b) => {
                let _ = write!(out, "{}", b);
            }
            Value::Void => out.push_str("null"),
            Value::List(items) => {
                out.push('[');
                for (i, item) in items.borrow().iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    stringify(item, out)?;
                }
                out.push(']');
            }
            Value::Object(object) => {
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
            Value::Variant(variant) => match (variant.name.as_str(), variant.values.as_slice()) {
                ("None", []) => out.push_str("null"),
                ("Some", [inner]) => stringify(inner, out)?,
                (name, []) => write_string(name, out),
                (name, values) => {
                    out.push('{');
                    write_string("type", out);
                    out.push(':');
                    write_string(name, out);
                    for (field, value) in variant.field_names.iter().zip(values) {
                        out.push(',');
                        write_string(field, out);
                        out.push(':');
                        stringify(value, out)?;
                    }
                    out.push('}');
                }
            },
            Value::Native(native) => match &*native.borrow() {
                Native::DateTime(datetime) => write_string(&datetime.to_string(), out),
                Native::File(_) => return Err("a File cannot be converted to JSON".to_string()),
            },
            other => {
                return Err(format!("a value of type {} cannot be converted to JSON", other.type_name()))
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_round_trip() {
        let timestamp = parse_datetime("2025-04-20", "YYYY-MM-DD").unwrap();
        assert_eq!(timestamp, 1_745_107_200);
        let datetime = datetime_from_timestamp(timestamp);
        assert_eq!(format_datetime(&datetime, "DD/MM/YYYY"), "20/04/2025");
        assert_eq!(format_datetime(&datetime, "yyyy-mm-dd hh:min:ss"), "2025-04-20 00:00:00");
    }

    #[test]
    fn datetime_rejects_invalid_dates() {
        assert!(parse_datetime("2025-02-30", "YYYY-MM-DD").is_err());
        assert!(parse_datetime("2025/02/03", "YYYY-MM-DD").is_err());
    }

    #[test]
    fn json_round_trip() {
        let value = json::parse(r#"{"name": "ezlang", "year": 2025, "tags": [1, 2.5, null]}"#).unwrap();
        let mut out = String::new();
        json::stringify(&value, &mut out).unwrap();
        assert_eq!(out, r#"{"name":"ezlang","year":2025,"tags":[1,2.5,null]}"#);
    }

    #[test]
    fn title_case() {
        assert!(is_title("Hello World"));
        assert!(!is_title("Hello world"));
        assert!(!is_title("123"));
    }
}

//! The `json` module.

use std::fmt::Write as _;

use super::*;

/// Calls the built-in function `name` of this module.
pub(super) fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
    match name {
        "json.parse" => {
            let text = str_arg(interpreter, &args, 0)?;
            Ok(match parse(text) {
                Ok(value) => ok(value),
                Err(error) => err(error),
            })
        }
        "json.stringify" => {
            let mut out = String::new();
            stringify(&args[0], &mut out).map_err(|message| interpreter.error(message))?;
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
        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
    }
}

/// The payload of a `Json` value if it is the variant `name` (`String('a')` → `'a'`).
fn json_payload(value: &Value, name: &str) -> Option<Value> {
    match value {
        Value::Variant(v) if v.name == name => v.values.first().cloned(),
        _ => None,
    }
}

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

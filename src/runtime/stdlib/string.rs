//! Methods of `string`.

use super::*;

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

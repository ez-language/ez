//! Functions available without import: `print`, `input`, `range`, conversions and `panic`.

use super::*;

/// Calls the built-in function `name` of this module.
pub(super) fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
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
        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
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

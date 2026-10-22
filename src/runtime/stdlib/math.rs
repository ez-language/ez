//! The `math` module.

use super::*;

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

/// Calls the built-in function `name` of this module.
pub(super) fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
    match name {
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
        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
    }
}

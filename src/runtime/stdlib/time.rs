//! The `time` module.

use super::*;

/// Calls the built-in function `name` of this module.
pub(super) fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
    match name {
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
        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
    }
}

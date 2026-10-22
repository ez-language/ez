//! The `file` module and the methods of `File`.

use super::*;

/// Calls the built-in function `name` of this module.
pub(super) fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
    match name {
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
        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
    }
}

/// Methods of an open `File`.
pub(super) fn method(interpreter: &mut Interpreter, handle: &mut FileHandle, name: &str, args: Vec<Value>) -> Exec<Value> {
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

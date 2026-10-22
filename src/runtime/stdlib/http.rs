//! The `http` module.

use super::*;

/// Calls the built-in function `name` of this module.
pub(super) fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
    match name {
        "http.get" | "http.delete" => {
            let url = str_arg(interpreter, &args, 0)?.to_string();
            Ok(http_request(&name[5..], &url, None))
        }
        "http.post" | "http.put" => {
            let url = str_arg(interpreter, &args, 0)?.to_string();
            let body = str_arg(interpreter, &args, 1)?.to_string();
            Ok(http_request(&name[5..], &url, Some(body)))
        }
        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
    }
}

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

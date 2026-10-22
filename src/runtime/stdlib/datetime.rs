//! The `datetime` module (UTC).

use super::*;

/// Calls the built-in function `name` of this module.
pub(super) fn call(interpreter: &mut Interpreter, name: &str, args: Vec<Value>) -> Exec<Value> {
    match name {
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
        other => Err(interpreter.error(format!("unknown built-in function '{}'", other))),
    }
}

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

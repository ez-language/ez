//! Sum types provided by the language and its standard library. Shared by the type checker
//! (which needs the field types) and the interpreter (which needs the variant and field names).

pub struct BuiltinSum {
    pub name: &'static str,
    /// Standard library module that exports the type; `None` for the prelude.
    pub module: Option<&'static str>,
    /// Number of generic parameters, written `T0`, `T1` in field types.
    pub params: usize,
    /// Variants with their fields (`name`, `type`).
    pub variants: &'static [(&'static str, &'static [(&'static str, &'static str)])],
}

const TEXT: &[(&str, &str)] = &[("text", "string")];
const PATH: &[(&str, &str)] = &[("path", "string")];
const PATH_MESSAGE: &[(&str, &str)] = &[("path", "string"), ("message", "string")];
const URL: &[(&str, &str)] = &[("url", "string")];
const URL_MESSAGE: &[(&str, &str)] = &[("url", "string"), ("message", "string")];

pub const BUILTIN_SUMS: &[BuiltinSum] = &[
    BuiltinSum {
        name: "Option",
        module: None,
        params: 1,
        variants: &[("Some", &[("value", "T0")]), ("None", &[])],
    },
    BuiltinSum {
        name: "Result",
        module: None,
        params: 2,
        variants: &[("Ok", &[("value", "T0")]), ("Err", &[("error", "T1")])],
    },
    BuiltinSum {
        name: "ParseError",
        module: None,
        params: 0,
        variants: &[("InvalidNumber", TEXT), ("OutOfRange", TEXT)],
    },
    BuiltinSum {
        name: "MathError",
        module: Some("math"),
        params: 0,
        variants: &[("Complex", &[]), ("Indeterminate", &[]), ("Infinite", &[])],
    },
    BuiltinSum {
        name: "FileError",
        module: Some("file"),
        params: 0,
        variants: &[
            ("NotFound", PATH),
            ("PermissionDenied", PATH),
            ("IsDirectory", PATH),
            ("InvalidUtf8", PATH),
            ("Io", PATH_MESSAGE),
        ],
    },
    BuiltinSum {
        name: "HttpError",
        module: Some("http"),
        params: 0,
        variants: &[
            ("InvalidUrl", URL),
            ("ConnectionFailed", URL_MESSAGE),
            ("Timeout", URL),
            ("Tls", URL_MESSAGE),
            ("InvalidResponse", URL_MESSAGE),
        ],
    },
    BuiltinSum {
        name: "JsonError",
        module: Some("json"),
        params: 0,
        variants: &[
            ("Syntax", &[("message", "string"), ("line", "int"), ("column", "int")]),
            ("NumberOutOfRange", &[("text", "string"), ("line", "int"), ("column", "int")]),
        ],
    },
    BuiltinSum {
        name: "Json",
        module: Some("json"),
        params: 0,
        variants: &[
            ("Null", &[]),
            ("Bool", &[("value", "boolean")]),
            ("Int", &[("value", "int")]),
            ("Float", &[("value", "float")]),
            ("String", &[("value", "string")]),
            ("Array", &[("items", "Json[]")]),
            ("Object", &[("fields", "JsonObject")]),
        ],
    },
    BuiltinSum {
        name: "DateError",
        module: Some("datetime"),
        params: 0,
        variants: &[
            ("PatternMismatch", &[("text", "string"), ("pattern", "string")]),
            ("InvalidDate", TEXT),
        ],
    },
];

pub fn builtin_sum(name: &str) -> &'static BuiltinSum {
    BUILTIN_SUMS
        .iter()
        .find(|sum| sum.name == name)
        .unwrap_or_else(|| panic!("unknown built-in sum type '{}'", name))
}

/// Functions of each standard library module.
pub fn stdlib_functions(module: &str) -> &'static [&'static str] {
    match module {
        "time" => &["sleep", "now", "monotonic"],
        "file" => &["open"],
        "datetime" => &["now", "today", "format", "parse"],
        "math" => &["abs", "sqrt", "pow", "floor", "ceil", "round"],
        "http" => &["get", "post", "put", "delete"],
        "json" => &["parse", "stringify", "get", "asString", "asInt", "asFloat", "asBool"],
        "regex" => &["match", "search", "findAll", "replace"],
        _ => &[],
    }
}

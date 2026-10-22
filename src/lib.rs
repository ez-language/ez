pub mod lexer;
pub mod parser;
pub mod semantic;
pub mod ir;
pub mod runtime;

pub mod error;
pub mod utils;

use std::io::{BufRead, Write};
use std::path::Path;

use error::EzError;
use runtime::interpreter::Interpreter;
use semantic::type_checker::{Checker, Program};

/// Parses and type-checks the file at `path` and every module it imports.
pub fn check_file(path: &Path) -> Result<Program, EzError> {
    Checker::new().check_file(path)
}

/// Checks and runs the file at `path`, using the process' standard input and output.
pub fn run_file(path: &Path) -> Result<(), EzError> {
    let program = check_file(path)?;
    let stdin = std::io::stdin();
    let mut interpreter = Interpreter::new(
        program,
        Box::new(std::io::stdout()),
        Box::new(stdin.lock()),
    );
    interpreter.run()
}

/// Checks and runs `source` as if it were the file `path` (imports are resolved relative to it).
pub fn run_source(
    source: &str,
    path: &Path,
    out: Box<dyn Write>,
    input: Box<dyn BufRead>,
) -> Result<(), EzError> {
    let program = Checker::new().check_source(path, source)?;
    Interpreter::new(program, out, input).run()
}

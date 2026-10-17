use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
ez - the ez programming language

Usage:
    ez <file.ez>          check and run a program
    ez run <file.ez>      check and run a program
    ez check <file.ez>    only check syntax and types
    ez help               show this message";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, file) = match args.as_slice() {
        [command, file] if command == "run" || command == "check" => (command.as_str(), file),
        [help] if help == "help" || help == "--help" || help == "-h" => {
            println!("{}", USAGE);
            return ExitCode::SUCCESS;
        }
        [file] => ("run", file),
        _ => {
            eprintln!("{}", USAGE);
            return ExitCode::from(2);
        }
    };

    let path = Path::new(file);
    let result = match command {
        "check" => ez::check_file(path).map(|_| ()),
        _ => ez::run_file(path),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", error);
            ExitCode::FAILURE
        }
    }
}

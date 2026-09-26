//! NCL command-line entry point.

use std::io::BufRead;

fn main() -> std::process::ExitCode {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("--version" | "-V") => {
            println!("ncl {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Some("--eval") => {
            let Some(source) = arguments.next() else {
                eprintln!("--eval requires a source string");
                return std::process::ExitCode::from(2);
            };
            let mut runtime = match ncl_runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("ncl: {error}");
                    return std::process::ExitCode::from(1);
                }
            };
            let result = runtime
                .compile(&source)
                .map(|value| runtime.format_result(value));
            match result {
                Ok(value) => {
                    println!("{value}");
                    std::process::ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("ncl: {error}");
                    std::process::ExitCode::from(1)
                }
            }
        }
        Some(mode @ ("--load" | "--script" | "--compile-file")) => {
            let Some(path) = arguments.next() else {
                eprintln!("{mode} requires a file path");
                return std::process::ExitCode::from(2);
            };
            let mut runtime = match ncl_runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("ncl: {error}");
                    return std::process::ExitCode::from(1);
                }
            };
            let result = if mode == "--compile-file" {
                runtime.compile_file(&path)
            } else {
                runtime.load_file(&path)
            };
            match result {
                Ok(value) if mode != "--script" => {
                    println!("{}", runtime.format_result(value));
                    std::process::ExitCode::SUCCESS
                }
                Ok(_) => std::process::ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("ncl: {error}");
                    std::process::ExitCode::from(1)
                }
            }
        }
        Some(argument) => {
            eprintln!("unsupported option: {argument}");
            std::process::ExitCode::from(2)
        }
        None => repl(),
    }
}

fn repl() -> std::process::ExitCode {
    let mut runtime = match ncl_runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("ncl: {error}");
            return std::process::ExitCode::from(1);
        }
    };
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();
    loop {
        eprint!("> ");
        line.clear();
        match input.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => match runtime.eval(&line) {
                Ok(value) => println!("{}", runtime.format_result(value)),
                Err(error) => eprintln!("ncl: {error}"),
            },
            Err(error) => {
                eprintln!("ncl: {error}");
                return std::process::ExitCode::from(1);
            }
        }
    }
    std::process::ExitCode::SUCCESS
}

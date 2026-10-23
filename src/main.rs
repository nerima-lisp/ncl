//! NCL command-line entry point.

use std::io::{BufRead, Write};

fn main() -> std::process::ExitCode {
    let (runtime_options, arguments) = match parse_runtime_options(std::env::args().skip(1)) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("ncl: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    let mut arguments = arguments.into_iter();
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
            if arguments.next().is_some() {
                eprintln!("--eval accepts exactly one source string");
                return std::process::ExitCode::from(2);
            }
            let mut runtime = match ncl_runtime::Runtime::with_options(runtime_options) {
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
            if arguments.next().is_some() {
                eprintln!("{mode} accepts exactly one file path");
                return std::process::ExitCode::from(2);
            }
            let mut runtime = match ncl_runtime::Runtime::with_options(runtime_options) {
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
        None => repl(runtime_options),
    }
}

fn parse_runtime_options(
    arguments: impl Iterator<Item = String>,
) -> Result<(ncl_runtime::RuntimeOptions, Vec<String>), String> {
    let mut options = ncl_runtime::RuntimeOptions::default();
    let mut remaining = Vec::new();
    let mut parsing_options = true;
    for argument in arguments {
        if parsing_options {
            if argument == "--opt-diagnostics" {
                options.opt_diagnostics = true;
                continue;
            }
            if let Some(mode) = argument.strip_prefix("--optimization=") {
                options.optimization = if mode == "default" {
                    ncl_runtime::OptimizationMode::Default
                } else if mode == "off" {
                    ncl_runtime::OptimizationMode::Off
                } else {
                    return Err(format!("unsupported optimization mode {mode:?}"));
                };
                continue;
            }
            parsing_options = false;
        }
        remaining.push(argument);
    }
    Ok((options, remaining))
}

fn repl(options: ncl_runtime::RuntimeOptions) -> std::process::ExitCode {
    let mut runtime = match ncl_runtime::Runtime::with_options(options) {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("ncl: {error}");
            return std::process::ExitCode::from(1);
        }
    };
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();
    let mut form = String::new();
    loop {
        eprint!("> ");
        if let Err(error) = std::io::stderr().flush() {
            eprintln!("ncl: {error}");
            return std::process::ExitCode::from(1);
        }
        line.clear();
        match input.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                form.push_str(&line);
                match runtime.eval(&form) {
                    Ok(value) => println!("{}", runtime.format_result(value)),
                    Err(error) if error.is_incomplete_read() => {
                        continue;
                    }
                    Err(error) => {
                        eprintln!("ncl: {error}");
                    }
                }
                form.clear();
            }
            Err(error) => {
                eprintln!("ncl: {error}");
                return std::process::ExitCode::from(1);
            }
        }
    }
    std::process::ExitCode::SUCCESS
}

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};

use ncl_runtime::{Runtime, RuntimeError};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() -> std::process::ExitCode {
    match run_inner() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(CliError::Usage(message)) => {
            eprintln!("{message}");
            std::process::ExitCode::from(2)
        }
        Err(CliError::Runtime(error)) => {
            eprintln!("{error}");
            std::process::ExitCode::from(1)
        }
        Err(CliError::Io(message)) => {
            eprintln!("{message}");
            std::process::ExitCode::from(1)
        }
    }
}

fn run_inner() -> Result<(), CliError> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_help();
        return Ok(());
    }
    if arguments
        .iter()
        .any(|argument| argument == "--version" || argument == "-V")
    {
        println!("ncl {VERSION}");
        return Ok(());
    }

    let mut evaluations = Vec::new();
    let mut file = None;
    let mut repl = false;
    let mut quiet = false;
    let mut compiled = false;
    let mut compile_only = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--eval" | "-e" => {
                index += 1;
                let Some(source) = arguments.get(index) else {
                    return Err(CliError::Usage(
                        "--eval requires a source string".to_string(),
                    ));
                };
                evaluations.push(source.clone());
            }
            "--file" | "-f" => {
                index += 1;
                let Some(path) = arguments.get(index) else {
                    return Err(CliError::Usage("--file requires a path".to_string()));
                };
                file = Some(path.clone());
            }
            "--repl" => repl = true,
            "--compiled" => compiled = true,
            "--compile" => compile_only = true,
            "--quiet" | "-q" => quiet = true,
            argument if argument.starts_with('-') => {
                return Err(CliError::Usage(format!("unknown option {argument}")));
            }
            path => {
                return Err(CliError::Usage(format!(
                    "unexpected argument {path}; use --file"
                )));
            }
        }
        index += 1;
    }

    if compile_only && compiled {
        return Err(CliError::Usage(
            "--compile cannot be combined with --compiled".to_string(),
        ));
    }
    if compile_only && repl {
        return Err(CliError::Usage(
            "--compile cannot be combined with --repl".to_string(),
        ));
    }
    if compile_only && evaluations.is_empty() && file.is_none() {
        return Err(CliError::Usage(
            "--compile requires --eval or --file".to_string(),
        ));
    }

    let runtime = Runtime::new();
    for source in &evaluations {
        if compile_only {
            print_compilation(&runtime, source, quiet)?;
        } else {
            print_values(&runtime, source, quiet, compiled)?;
        }
    }
    if let Some(ref path) = file {
        let source = fs::read_to_string(&path)
            .map_err(|error| CliError::Io(format!("cannot read {path}: {error}")))?;
        if compile_only {
            print_compilation(&runtime, &source, quiet)?;
        } else {
            print_values(&runtime, &source, quiet, compiled)?;
        }
    }
    if repl || (evaluations.is_empty() && file.is_none()) {
        repl_loop(&runtime, quiet, compiled)?;
    }
    Ok(())
}

fn print_compilation(runtime: &Runtime, source: &str, quiet: bool) -> Result<(), CliError> {
    let forms = runtime.compile_source(source).map_err(CliError::Runtime)?;
    if !quiet {
        let function_count = forms
            .iter()
            .map(|form| form.function_count())
            .sum::<usize>();
        let instruction_count = forms
            .iter()
            .map(|form| form.instruction_count())
            .sum::<usize>();
        println!(
            "compiled {} form(s), {} function(s), {} instruction(s)",
            forms.len(),
            function_count,
            instruction_count
        );
    }
    Ok(())
}

fn print_values(
    runtime: &Runtime,
    source: &str,
    quiet: bool,
    compiled: bool,
) -> Result<(), CliError> {
    let values = if compiled {
        runtime.eval_compiled_source(source)
    } else {
        runtime.eval_source(source)
    }
    .map_err(CliError::Runtime)?;
    for value in values {
        if !quiet {
            println!("{value}");
        }
    }
    Ok(())
}

fn repl_loop(runtime: &Runtime, quiet: bool, compiled: bool) -> Result<(), CliError> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();
    loop {
        if !quiet {
            print!("ncl> ");
            io::stdout()
                .flush()
                .map_err(|error| CliError::Io(error.to_string()))?;
        }
        line.clear();
        if input
            .read_line(&mut line)
            .map_err(|error| CliError::Io(error.to_string()))?
            == 0
        {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let result = if compiled {
            runtime.eval_compiled_source(&line)
        } else {
            runtime.eval_source(&line)
        };
        match result {
            Ok(values) => {
                if !quiet {
                    for value in values {
                        println!("{value}");
                    }
                }
            }
            Err(error) => eprintln!("{error}"),
        }
    }
    Ok(())
}

fn print_help() {
    println!(
        "ncl {VERSION}

Usage:
  ncl --eval EXPR
  ncl --file PATH
  ncl --repl

Options:
  -e, --eval EXPR   Evaluate source text
  -f, --file PATH   Evaluate a source file
      --repl        Start the line-oriented REPL
      --compiled     Execute input through the bytecode compiler and VM
      --compile      Compile input without executing it
  -q, --quiet       Suppress value output and REPL prompts
  -h, --help        Show this help
  -V, --version     Show the version"
    );
}

enum CliError {
    Usage(String),
    Runtime(RuntimeError),
    Io(String),
}

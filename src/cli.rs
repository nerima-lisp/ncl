use std::env;
use std::fs;
use std::io::{self, Write};

use ncl_runtime::{Runtime, RuntimeError};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runs the NCL command-line interface and returns its process status.
#[must_use]
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

    let options = CliOptions::parse(&arguments)?;
    let runtime = Runtime::new();
    for source in &options.evaluations {
        print_values(&runtime, source, options.quiet, options.compiled)?;
    }
    if let Some(path) = &options.file {
        let source = fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read {path}: {error}")))?;
        print_values(&runtime, &source, options.quiet, options.compiled)?;
    }
    if options.repl || (options.evaluations.is_empty() && options.file.is_none()) {
        repl_loop(&runtime, options.quiet, options.compiled)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct CliOptions {
    evaluations: Vec<String>,
    file: Option<String>,
    repl: bool,
    quiet: bool,
    compiled: bool,
}

impl CliOptions {
    fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let mut options = Self::default();
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
                    options.evaluations.push(source.clone());
                }
                "--file" | "-f" => {
                    index += 1;
                    let Some(path) = arguments.get(index) else {
                        return Err(CliError::Usage("--file requires a path".to_string()));
                    };
                    options.file = Some(path.clone());
                }
                "--repl" => options.repl = true,
                "--compiled" => options.compiled = true,
                "--quiet" | "-q" => options.quiet = true,
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
        Ok(options)
    }
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
    let mut line = String::new();
    loop {
        if !quiet {
            print!("ncl> ");
            io::stdout()
                .flush()
                .map_err(|error| CliError::Io(error.to_string()))?;
        }
        line.clear();
        if io::stdin()
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
  -q, --quiet       Suppress value output and REPL prompts
  -h, --help        Show this help
  -V, --version     Show the version"
    );
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Runtime(RuntimeError),
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::CliOptions;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_all_execution_options() -> Result<(), String> {
        let options = CliOptions::parse(&arguments(&[
            "-e",
            "(+ 1 2)",
            "--eval",
            "42",
            "-f",
            "input.lisp",
            "--repl",
            "--compiled",
            "-q",
        ]))
        .map_err(|error| format!("valid options should parse: {error:?}"))?;

        assert_eq!(options.evaluations, ["(+ 1 2)", "42"]);
        assert_eq!(options.file.as_deref(), Some("input.lisp"));
        assert!(options.repl);
        assert!(options.compiled);
        assert!(options.quiet);
        Ok(())
    }

    #[test]
    fn rejects_missing_values_and_unexpected_arguments() -> Result<(), String> {
        for (values, expected) in [
            (&["--eval"][..], "--eval requires a source string"),
            (&["--file"][..], "--file requires a path"),
            (&["input.lisp"][..], "unexpected argument input.lisp"),
        ] {
            let error = CliOptions::parse(&arguments(values))
                .err()
                .ok_or_else(|| "options should fail".to_string())?;
            assert!(format!("{error:?}").contains(expected));
        }
        Ok(())
    }

    #[test]
    fn rejects_unknown_options() -> Result<(), String> {
        let error = CliOptions::parse(&arguments(&["--unknown"]))
            .err()
            .ok_or_else(|| "unknown options should fail".to_string())?;
        assert!(format!("{error:?}").contains("unknown option --unknown"));
        Ok(())
    }
}

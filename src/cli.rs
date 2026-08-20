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
    match parse_arguments(&arguments)? {
        CliCommand::Help => {
            print_help();
            Ok(())
        }
        CliCommand::Version => {
            println!("ncl {VERSION}");
            Ok(())
        }
        CliCommand::Run(options) => run_options(options),
    }
}

fn run_options(options: Options) -> Result<(), CliError> {
    let Options {
        evaluations,
        file,
        repl,
        quiet,
        compiled,
        compile_only,
    } = options;

    let runtime = Runtime::new();
    for source in &evaluations {
        if compile_only {
            print_compilation(&runtime, source, quiet)?;
        } else {
            print_values(&runtime, source, quiet, compiled)?;
        }
    }
    if let Some(ref path) = file {
        let source = fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read {path}: {error}")))?;
        if compile_only {
            print_compilation(&runtime, &source, quiet)?;
        } else {
            print_values(&runtime, &source, quiet, compiled)?;
        }
    }
    if !compile_only && (repl || (evaluations.is_empty() && file.is_none())) {
        repl_loop(&runtime, quiet, compiled)?;
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    evaluations: Vec<String>,
    file: Option<String>,
    repl: bool,
    quiet: bool,
    compiled: bool,
    compile_only: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum CliCommand {
    Help,
    Version,
    Run(Options),
}

fn parse_arguments(arguments: &[String]) -> Result<CliCommand, CliError> {
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        return Ok(CliCommand::Help);
    }
    if arguments
        .iter()
        .any(|argument| argument == "--version" || argument == "-V")
    {
        return Ok(CliCommand::Version);
    }

    let mut options = Options {
        evaluations: Vec::new(),
        file: None,
        repl: false,
        quiet: false,
        compiled: false,
        compile_only: false,
    };
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
            "--compile" => options.compile_only = true,
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

    if options.compile_only && options.compiled {
        return Err(CliError::Usage(
            "--compile cannot be combined with --compiled".to_string(),
        ));
    }
    if options.compile_only && options.repl {
        return Err(CliError::Usage(
            "--compile cannot be combined with --repl".to_string(),
        ));
    }
    if options.compile_only && options.evaluations.is_empty() && options.file.is_none() {
        return Err(CliError::Usage(
            "--compile requires --eval or --file".to_string(),
        ));
    }

    Ok(CliCommand::Run(options))
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

#[derive(Debug, Eq, PartialEq)]
enum CliError {
    Usage(String),
    Runtime(RuntimeError),
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::{CliCommand, Options, parse_arguments};

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(ToString::to_string).collect()
    }

    fn assert_usage(values: &[&str], expected: &str) {
        assert_eq!(
            parse_arguments(&arguments(values)),
            Err(super::CliError::Usage(expected.to_string()))
        );
    }

    #[test]
    fn parses_execution_options_and_aliases() {
        let command = parse_arguments(&arguments(&[
            "-e",
            "(+ 1 2)",
            "--eval",
            "(+ 3 4)",
            "-f",
            "program.ncl",
            "--repl",
            "--compiled",
            "-q",
        ]));

        assert_eq!(
            command,
            Ok(CliCommand::Run(Options {
                evaluations: vec!["(+ 1 2)".to_string(), "(+ 3 4)".to_string()],
                file: Some("program.ncl".to_string()),
                repl: true,
                quiet: true,
                compiled: true,
                compile_only: false,
            }))
        );
    }

    #[test]
    fn help_and_version_are_detected_before_other_arguments() {
        assert_eq!(
            parse_arguments(&arguments(&["--help", "--unknown"])),
            Ok(CliCommand::Help)
        );
        assert_eq!(
            parse_arguments(&arguments(&["--version", "--unknown"])),
            Ok(CliCommand::Version)
        );
    }

    #[test]
    fn reports_missing_values_unknown_options_and_positional_paths() {
        assert_usage(&["--eval"], "--eval requires a source string");
        assert_usage(&["--file"], "--file requires a path");
        assert_usage(&["--unknown"], "unknown option --unknown");
        assert_usage(
            &["program.ncl"],
            "unexpected argument program.ncl; use --file",
        );
    }
}

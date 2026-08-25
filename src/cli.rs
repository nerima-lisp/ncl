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

    let options = parse_arguments(&arguments)?;
    let CliOptions {
        evaluations,
        file,
        repl,
        quiet,
        compiled,
    } = options;
    let runtime = Runtime::new();
    for source in &evaluations {
        print_values(&runtime, source, quiet, compiled)?;
    }
    if let Some(ref path) = file {
        let source = fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("cannot read {path}: {error}")))?;
        print_values(&runtime, &source, quiet, compiled)?;
    }
    if repl || (evaluations.is_empty() && file.is_none()) {
        repl_loop(&runtime, quiet, compiled)?;
    }
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CliOptions {
    evaluations: Vec<String>,
    file: Option<String>,
    repl: bool,
    quiet: bool,
    compiled: bool,
}

fn parse_arguments(arguments: &[String]) -> Result<CliOptions, CliError> {
    let mut options = CliOptions::default();
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
    let input = stdin.lock();
    let stdout = io::stdout();
    let output = stdout.lock();
    let stderr = io::stderr();
    let errors = stderr.lock();
    repl_loop_with_io(runtime, quiet, compiled, input, output, errors)
}

fn repl_loop_with_io<R, W, E>(
    runtime: &Runtime,
    quiet: bool,
    compiled: bool,
    mut input: R,
    mut output: W,
    mut errors: E,
) -> Result<(), CliError>
where
    R: BufRead,
    W: Write,
    E: Write,
{
    let mut line = String::new();
    loop {
        if !quiet {
            write!(output, "ncl> ")
                .and_then(|()| output.flush())
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
                        writeln!(output, "{value}")
                            .map_err(|error| CliError::Io(error.to_string()))?;
                    }
                }
            }
            Err(error) => {
                writeln!(errors, "{error}").map_err(|error| CliError::Io(error.to_string()))?
            }
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
    use std::io::Cursor;

    use super::{CliError, CliOptions, parse_arguments, print_values, repl_loop_with_io};
    use ncl_runtime::Runtime;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parses_all_options_and_aliases() {
        let parsed = parse_arguments(&args(&[
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
        .expect("valid arguments");
        assert_eq!(
            parsed,
            CliOptions {
                evaluations: vec!["(+ 1 2)".into(), "42".into()],
                file: Some("input.lisp".into()),
                repl: true,
                quiet: true,
                compiled: true,
            }
        );
    }

    #[test]
    fn accepts_empty_arguments() {
        assert!(matches!(parse_arguments(&[]), Ok(options) if options == CliOptions::default()));
    }

    #[test]
    fn reports_missing_values_and_unknown_arguments() {
        for (input, message) in [
            (&["--eval"][..], "--eval requires a source string"),
            (&["--file"][..], "--file requires a path"),
            (&["--unknown"][..], "unknown option --unknown"),
            (
                &["input.lisp"][..],
                "unexpected argument input.lisp; use --file",
            ),
        ] {
            assert!(matches!(
                parse_arguments(&args(input)),
                Err(CliError::Usage(actual)) if actual == message
            ));
        }
    }

    #[test]
    fn reports_evaluation_errors_in_both_execution_modes() {
        let runtime = Runtime::new();
        for compiled in [false, true] {
            let result = print_values(&runtime, "(car 1)", true, compiled);
            assert!(
                matches!(result, Err(CliError::Runtime(_))),
                "compiled={compiled}"
            );
        }
    }

    #[test]
    fn repl_evaluates_values_skips_blank_lines_and_reports_errors() {
        let runtime = Runtime::new();
        let mut output = Vec::new();
        let mut errors = Vec::new();

        repl_loop_with_io(
            &runtime,
            false,
            false,
            Cursor::new("\n(+ 1 2)\n(car 1)\n"),
            &mut output,
            &mut errors,
        )
        .expect("REPL input should be processed");

        let output = String::from_utf8(output).expect("REPL output is UTF-8");
        let errors = String::from_utf8(errors).expect("REPL errors are UTF-8");
        assert_eq!(output.matches("ncl> ").count(), 4);
        assert!(output.contains("3\n"));
        assert!(!errors.is_empty());
    }

    #[test]
    fn compiled_quiet_repl_suppresses_output_and_errors_are_empty_on_success() {
        let runtime = Runtime::new();
        let mut output = Vec::new();
        let mut errors = Vec::new();

        repl_loop_with_io(
            &runtime,
            true,
            true,
            Cursor::new("(+ 2 3)\n"),
            &mut output,
            &mut errors,
        )
        .expect("compiled REPL input should be processed");

        assert!(output.is_empty());
        assert!(errors.is_empty());
    }
}

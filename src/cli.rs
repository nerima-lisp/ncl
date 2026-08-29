use std::env;
use std::fs;

use ncl_runtime::Runtime;

mod error;
mod options;
mod output;
mod repl;

use error::CliError;
use options::CliOptions;
use output::{print_help, print_values};
use repl::repl_loop;

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

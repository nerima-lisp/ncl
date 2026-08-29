use std::io::{self, Write};

use ncl_runtime::Runtime;

use super::VERSION;
use super::error::CliError;

pub(super) fn print_values(
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

pub(super) fn repl_loop(runtime: &Runtime, quiet: bool, compiled: bool) -> Result<(), CliError> {
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

pub(super) fn print_help() {
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

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

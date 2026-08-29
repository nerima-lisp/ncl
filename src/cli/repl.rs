use std::borrow::Cow;
use std::io::{self, IsTerminal, Write};

use ncl_runtime::Runtime;
use ncl_syntax::{ReadError, ReadErrorKind};
use reedline::{Prompt, PromptEditMode, PromptHistorySearch, ValidationResult, Validator};

use super::error::CliError;
use interactive::interactive_repl_loop;

mod interactive;

/// Runs the REPL, reading from a real terminal with line editing and
/// multi-line continuation via reedline, or falling back to a plain
/// buffered stdin reader (also multi-line-aware) when stdin or stdout is
/// redirected, since reedline requires raw terminal access to either.
pub(super) fn repl_loop(runtime: &Runtime, quiet: bool, compiled: bool) -> Result<(), CliError> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        interactive_repl_loop(runtime, quiet, compiled)
    } else {
        piped_repl_loop(runtime, quiet, compiled)
    }
}

fn piped_repl_loop(runtime: &Runtime, quiet: bool, compiled: bool) -> Result<(), CliError> {
    let mut buffer = String::new();
    loop {
        if buffer.is_empty() && !quiet {
            print!("ncl> ");
            io::stdout()
                .flush()
                .map_err(|error| CliError::Io(error.to_string()))?;
        }
        let mut line = String::new();
        let bytes_read = io::stdin()
            .read_line(&mut line)
            .map_err(|error| CliError::Io(error.to_string()))?;
        if bytes_read == 0 {
            if !buffer.trim().is_empty() {
                evaluate_repl_input(runtime, &buffer, quiet, compiled);
            }
            break;
        }
        buffer.push_str(&line);
        if buffer.trim().is_empty() {
            buffer.clear();
        } else if !is_incomplete(&buffer) {
            evaluate_repl_input(runtime, &buffer, quiet, compiled);
            buffer.clear();
        }
    }
    Ok(())
}

fn evaluate_repl_input(runtime: &Runtime, source: &str, quiet: bool, compiled: bool) {
    let result = if compiled {
        runtime.eval_compiled_source(source)
    } else {
        runtime.eval_source(source)
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

/// True when `source` ends mid-form (an unclosed list, string, etc.) rather
/// than containing a genuine syntax error, so the REPL should keep reading
/// more lines instead of reporting a failure.
fn is_incomplete(source: &str) -> bool {
    matches!(
        ncl_syntax::read(source),
        Err(ReadError {
            kind: ReadErrorKind::UnexpectedEnd { .. },
            ..
        })
    )
}

struct FormValidator;

impl Validator for FormValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if is_incomplete(line) {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

struct NclPrompt {
    quiet: bool,
}

impl Prompt for NclPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        if self.quiet {
            Cow::Borrowed("")
        } else {
            Cow::Borrowed("ncl> ")
        }
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        if self.quiet {
            Cow::Borrowed("")
        } else {
            Cow::Borrowed("...  ")
        }
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Owned(format!("(reverse-search: {}) ", history_search.term))
    }
}

#[cfg(test)]
mod tests;

use ncl_runtime::Runtime;
use reedline::{Reedline, Signal};

use super::{CliError, FormValidator, NclPrompt, evaluate_repl_input};

// Reedline enables raw terminal mode on first read_line() call, which fails
// against a non-tty stdin/stdout (repl_loop only reaches this function when
// both are real terminals) -- there is no way to drive it under a headless
// test harness, so this function is excluded from the coverage gate in
// .github/workflows/ci.yml rather than tested directly. FormValidator,
// NclPrompt, and is_incomplete, which carry this loop's actual decision
// logic, are ordinary functions tested in repl/tests.rs without needing a
// terminal at all.
pub(super) fn interactive_repl_loop(
    runtime: &Runtime,
    quiet: bool,
    compiled: bool,
) -> Result<(), CliError> {
    let mut line_editor = Reedline::create().with_validator(Box::new(FormValidator));
    let prompt = NclPrompt { quiet };
    loop {
        match line_editor.read_line(&prompt) {
            Ok(Signal::Success(buffer)) => {
                if !buffer.trim().is_empty() {
                    evaluate_repl_input(runtime, &buffer, quiet, compiled);
                }
            }
            Ok(Signal::CtrlC) => {}
            Ok(Signal::CtrlD) => break,
            Err(error) => return Err(CliError::Io(error.to_string())),
        }
    }
    Ok(())
}

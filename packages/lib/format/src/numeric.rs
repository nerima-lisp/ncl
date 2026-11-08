mod floating;
mod radix;

use crate::dispatch::NumericDirective;
use crate::executor::{ExecutionState, FormatError, FormatOutput};
use crate::model::Directive;

pub fn execute(
    kind: NumericDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let parameters = state.parameters(directive)?;
    match kind {
        NumericDirective::Radix(kind) => {
            radix::execute(kind, directive, state, output, &parameters)
        }
        NumericDirective::Floating(kind) => {
            floating::execute(kind, directive, state, output, &parameters)
        }
    }
}

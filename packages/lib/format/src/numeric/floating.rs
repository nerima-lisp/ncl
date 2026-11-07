mod common;
mod directives;

use crate::executor::{ExecutionState, FormatError, FormatOutput};
use crate::model::{Directive, Parameter};

pub(super) fn execute(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[Parameter],
) -> Result<(), FormatError> {
    directives::execute(directive, state, output, parameters)
}

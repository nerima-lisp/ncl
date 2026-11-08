mod common;
mod directives;

use crate::dispatch::FloatingDirective;
use crate::executor::{ExecutionState, FormatError, FormatOutput};
use crate::model::{Directive, Parameter};

pub(super) fn execute(
    kind: FloatingDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[Parameter],
) -> Result<(), FormatError> {
    directives::execute(kind, directive, state, output, parameters)
}

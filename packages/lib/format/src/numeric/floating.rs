mod common;
mod directives;

use crate::dispatch::FloatingDirective;
use crate::executor::{ExecutionState, FormatError, FormatOutput, ResolvedParameter};
use crate::model::Directive;

pub(super) fn execute(
    kind: FloatingDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
) -> Result<(), FormatError> {
    directives::execute(kind, directive, state, output, parameters)
}

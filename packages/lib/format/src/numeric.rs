mod floating;
mod radix;

use crate::executor::{ExecutionState, FormatError, FormatOutput};
use crate::model::{Directive, DirectiveKind};

pub(crate) fn execute(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let parameters = state.parameters(directive)?;
    match directive.kind {
        DirectiveKind::D
        | DirectiveKind::B
        | DirectiveKind::O
        | DirectiveKind::X
        | DirectiveKind::R => radix::execute(directive, state, output, &parameters),
        DirectiveKind::F | DirectiveKind::E | DirectiveKind::G | DirectiveKind::Dollar => {
            floating::execute(directive, state, output, &parameters)
        }
        _ => Err(radix::invalid(directive)),
    }
}

mod floating;
mod radix;

use crate::executor::{ExecutionState, FormatError, FormatOutput};
use crate::model::{Directive, DirectiveKind};

pub fn execute(
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
        DirectiveKind::A
        | DirectiveKind::S
        | DirectiveKind::W
        | DirectiveKind::C
        | DirectiveKind::Percent
        | DirectiveKind::Ampersand
        | DirectiveKind::Bar
        | DirectiveKind::Tilde
        | DirectiveKind::Newline
        | DirectiveKind::Underscore
        | DirectiveKind::LogicalBlock
        | DirectiveKind::Indent
        | DirectiveKind::Slash
        | DirectiveKind::Tab
        | DirectiveKind::Justification
        | DirectiveKind::Star
        | DirectiveKind::Conditional
        | DirectiveKind::Iteration
        | DirectiveKind::Recursive
        | DirectiveKind::CaseConversion
        | DirectiveKind::Plural
        | DirectiveKind::UpArrow => Err(radix::invalid(directive)),
    }
}

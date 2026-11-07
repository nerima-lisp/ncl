//! Layout controls implemented by the FORMAT output cursor.

mod justification;
mod layout;
mod named;

use crate::executor::{ExecutionState, FormatOutput};
use crate::model::{Directive, DirectiveKind};
use crate::{FormatError, FormatErrorKind};
use ncl_printer::PrintOptions;

pub(crate) fn execute(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    match directive.kind {
        DirectiveKind::Underscore | DirectiveKind::Indent | DirectiveKind::LogicalBlock => {
            output.pretty = PrintOptions::from_specials(state.ctx, state.runtime).pretty();
        }
        _ => {}
    }
    match directive.kind {
        DirectiveKind::Underscore => layout::execute_underscore(directive, output),
        DirectiveKind::Indent => layout::execute_indent(directive, state, output),
        DirectiveKind::Tab => layout::execute_tab(directive, state, output),
        DirectiveKind::LogicalBlock => layout::execute_logical_block(directive, state, output),
        DirectiveKind::Justification => {
            justification::execute_justification(directive, state, output)
        }
        DirectiveKind::Slash => named::execute(directive, state, output),
        _ => Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        )),
    }
}

//! Layout controls implemented by the FORMAT output cursor.

mod justification;
mod layout;
mod named;

use crate::FormatError;
use crate::dispatch::PrettyDirective;
use crate::executor::{ExecutionState, FormatOutput};
use crate::model::Directive;
use ncl_printer::PrintOptions;

pub fn execute(
    kind: PrettyDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    if matches!(
        kind,
        PrettyDirective::Underscore | PrettyDirective::Indent | PrettyDirective::LogicalBlock
    ) {
        output.pretty = PrintOptions::from_specials(state.ctx, state.runtime).pretty();
    }
    match kind {
        PrettyDirective::Underscore => layout::execute_underscore(directive, state, output),
        PrettyDirective::Indent => layout::execute_indent(directive, state, output),
        PrettyDirective::Tab => layout::execute_tab(directive, state, output),
        PrettyDirective::LogicalBlock => layout::execute_logical_block(directive, state, output),
        PrettyDirective::Justification => {
            justification::execute_justification(directive, state, output)
        }
        PrettyDirective::Slash => named::execute(directive, state, output),
    }
}

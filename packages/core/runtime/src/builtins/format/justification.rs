#![allow(clippy::wildcard_imports)]

use super::*;

pub(super) fn format_justification_pieces(
    clauses: &[&[char]],
    arguments: &[Value],
    colon_iteration_last: bool,
) -> Result<(Vec<String>, usize), RuntimeError> {
    let mut pieces = Vec::new();
    let mut argument_index = 0;
    for clause in clauses {
        let (formatted, consumed, termination) =
            format_control_characters(clause, &arguments[argument_index..], colon_iteration_last)?;
        argument_index += consumed;
        if termination.is_some() {
            break;
        }
        pieces.push(formatted);
    }
    Ok((pieces, argument_index))
}

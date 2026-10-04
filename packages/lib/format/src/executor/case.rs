use ncl_printer::StringSink;

use crate::{ControlPart, Directive, DirectiveKind};

use super::{ExecutionState, FormatError, execute_parts, matching};

pub(super) fn execute_case_group(
    parts: &[ControlPart],
    index: usize,
    end: usize,
    directive: &Directive,
    state: &mut ExecutionState<'_, '_>,
) -> Result<Option<usize>, FormatError> {
    let close = matching(
        parts,
        index,
        end,
        DirectiveKind::ParenOpen,
        DirectiveKind::ParenClose,
    )?;
    let mut local = StringSink::new();
    let mut nested = ExecutionState {
        arguments: state.arguments,
        argument_index: state.argument_index,
        ctx: state.ctx,
        runtime: state.runtime,
        sink: &mut local,
        line_start: state.line_start,
        column: state.column,
        escape: state.escape,
        remaining_override: state.remaining_override,
        caller: state.caller.clone(),
        pretty: state.pretty.clone(),
    };
    execute_parts(parts, index + 1, close, &mut nested)?;
    let text = local.into_string();
    let text = match (directive.colon, directive.at_sign) {
        (false, false) => text.to_lowercase(),
        (true, false) => capitalize_words(&text),
        (false, true) => capitalize_first_word(&text),
        (true, true) => text.to_uppercase(),
    };
    state.sink.write_str(&text).map_err(FormatError::from)?;
    *state.line_start = text.ends_with('\n');
    Ok(Some(close + 1))
}

fn capitalize_words(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut word_start = true;
    for character in text.chars() {
        if character.is_alphabetic() {
            if word_start {
                result.extend(character.to_uppercase());
                word_start = false;
            } else {
                result.extend(character.to_lowercase());
            }
        } else {
            word_start = !character.is_alphanumeric();
            result.push(character);
        }
    }
    result
}

fn capitalize_first_word(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut first = true;
    for character in text.chars() {
        if first && character.is_alphabetic() {
            result.extend(character.to_uppercase());
            first = false;
        } else if !first && character.is_alphabetic() {
            result.extend(character.to_lowercase());
        } else {
            result.push(character);
        }
    }
    result
}

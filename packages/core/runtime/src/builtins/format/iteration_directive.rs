#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn format_iteration_directive(
    state: &mut FormatControlState<'_>,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<(), RuntimeError> {
    let body_end = format_iteration_end(state.characters, *state.character_index)?;
    let body = &state.characters[*state.character_index..body_end];
    *state.character_index = body_end + 2;
    let limit = format_iteration_limit(parameters)?;
    if at_sign_modifier {
        let (formatted, consumed) = format_iteration(
            body,
            &state.arguments[*state.argument_index..],
            colon_modifier,
            limit,
        )?;
        state.output.push_str(&formatted);
        *state.argument_index += consumed;
    } else {
        let list = format_argument("~{", state.arguments, state.argument_index)?;
        let list = list
            .list_items()
            .ok_or_else(|| type_error("format", "a list for ~{", list))?;
        let (formatted, _) = format_iteration(body, &list, colon_modifier, limit)?;
        state.output.push_str(&formatted);
    }
    Ok(())
}

pub(super) fn format_iteration(
    body: &[char],
    arguments: &[Value],
    colon_modifier: bool,
    limit: Option<usize>,
) -> Result<(String, usize), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    let mut repetitions = 0;
    while argument_index < arguments.len() && limit.is_none_or(|limit| repetitions < limit) {
        let (consumed, termination) = if colon_modifier {
            let nested_arguments = arguments[argument_index].list_items().ok_or_else(|| {
                type_error(
                    "format",
                    "a list element for ~:{",
                    &arguments[argument_index],
                )
            })?;
            let (formatted, consumed, termination) = format_control_characters(
                body,
                &nested_arguments,
                argument_index + 1 >= arguments.len(),
            )?;
            output.push_str(&formatted);
            (consumed, termination)
        } else {
            let (formatted, consumed, termination) =
                format_control_characters(body, &arguments[argument_index..], false)?;
            output.push_str(&formatted);
            (consumed, termination)
        };
        argument_index += if colon_modifier { 1 } else { consumed.max(1) };
        repetitions += 1;
        if let Some(termination) = termination {
            if colon_modifier && !termination.colon_modifier {
                continue;
            }
            break;
        }
    }
    Ok((output, argument_index))
}

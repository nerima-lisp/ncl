#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn format_choice_directive(
    state: &mut FormatControlState<'_>,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<Option<FormatTermination>, RuntimeError> {
    let body_end = format_choice_end(state.characters, *state.character_index)?;
    let body = &state.characters[*state.character_index..body_end];
    *state.character_index = body_end + 2;
    let clauses = format_choice_clauses(body)?;
    if colon_modifier && (at_sign_modifier || clauses.len() != 2) {
        return Err(RuntimeError::InvalidForm {
            message: "invalid format choice modifiers or clause count".to_string(),
            span: None,
        });
    }
    if at_sign_modifier && clauses.len() != 1 {
        return Err(RuntimeError::InvalidForm {
            message: "at-sign format choice needs one clause".to_string(),
            span: None,
        });
    }
    if (colon_modifier || at_sign_modifier) && !parameters.is_empty()
        || !colon_modifier && !at_sign_modifier && parameters.len() > 1
    {
        return Err(RuntimeError::InvalidForm {
            message: "invalid format choice parameters".to_string(),
            span: None,
        });
    }
    let selected_index = if colon_modifier {
        Some(usize::from(
            format_argument("~[", state.arguments, state.argument_index)?.is_truthy(),
        ))
    } else if at_sign_modifier {
        let selector = state.arguments.get(*state.argument_index).ok_or_else(|| {
            RuntimeError::InvalidForm {
                message: "format directive ~[ needs another argument".to_string(),
                span: None,
            }
        })?;
        if selector.is_truthy() {
            Some(0)
        } else {
            *state.argument_index += 1;
            None
        }
    } else {
        let index = if matches!(
            parameters.first().copied(),
            Some(FormatParameter::Number(_) | FormatParameter::Character(_))
        ) {
            format_parameter_number(parameters, 0, 0)?
        } else {
            integer_argument(
                "format choice",
                format_argument("~[", state.arguments, state.argument_index)?,
            )?
        };
        usize::try_from(index).ok()
    };
    let clause = selected_index
        .and_then(|index| {
            clauses
                .get(index)
                .or_else(|| clauses.iter().find(|(_, default)| *default))
        })
        .or_else(|| {
            (!colon_modifier && !at_sign_modifier)
                .then(|| clauses.iter().find(|(_, default)| *default))
                .flatten()
        });
    if let Some((clause, _)) = clause {
        let (formatted, consumed, termination) = format_control_characters(
            clause,
            &state.arguments[*state.argument_index..],
            state.colon_iteration_last,
        )?;
        state.output.push_str(&formatted);
        *state.argument_index += consumed;
        return Ok(termination);
    }
    Ok(None)
}

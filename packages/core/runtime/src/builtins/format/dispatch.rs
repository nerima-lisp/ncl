use super::*;

pub(super) fn format_control_characters(
    characters: &[char],
    arguments: &[Value],
    colon_iteration_last: bool,
) -> Result<(String, usize, Option<FormatTermination>), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    let mut character_index = 0;
    while character_index < characters.len() {
        let character = characters[character_index];
        character_index += 1;
        if character != '~' {
            output.push(character);
            continue;
        }
        let FormatDirective {
            parameters,
            directive,
            colon_modifier,
            at_sign_modifier,
        } = parse_format_directive(
            characters,
            &mut character_index,
            arguments,
            &mut argument_index,
        )?;
        if format_simple_directive(
            directive,
            &mut output,
            arguments,
            &mut argument_index,
            &parameters,
            colon_modifier,
            at_sign_modifier,
        )? {
            continue;
        }
        let termination = format_non_simple_directive(
            &mut FormatControlState {
                characters,
                arguments,
                output: &mut output,
                argument_index: &mut argument_index,
                character_index: &mut character_index,
                colon_iteration_last,
            },
            directive,
            &parameters,
            colon_modifier,
            at_sign_modifier,
        )?;
        if let Some(termination) = termination {
            return Ok((output, argument_index, Some(termination)));
        }
    }
    Ok((output, argument_index, None))
}

pub(super) fn format_non_simple_directive(
    state: &mut FormatControlState<'_>,
    directive: char,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<Option<FormatTermination>, RuntimeError> {
    match directive {
        'A' | 'S' => state.output.push_str(&format_value_directive(
            directive,
            state.arguments,
            state.argument_index,
            parameters,
            colon_modifier,
            at_sign_modifier,
        )?),
        '(' => return format_case_directive(state, parameters, colon_modifier, at_sign_modifier),
        'D' | 'B' | 'O' | 'X' | 'F' | 'G' | 'E' | '$' => {
            state.output.push_str(&format_numeric_directive(
                directive,
                state.arguments,
                state.argument_index,
                parameters,
                colon_modifier,
                at_sign_modifier,
            )?);
        }
        '?' | '^' => {
            return format_nested_or_escape_directive(
                directive,
                state,
                parameters,
                colon_modifier,
                at_sign_modifier,
            );
        }
        '{' => format_iteration_directive(state, parameters, colon_modifier, at_sign_modifier)?,
        '[' => return format_choice_directive(state, parameters, colon_modifier, at_sign_modifier),
        '<' => format_justification_directive(state, parameters, colon_modifier, at_sign_modifier)?,
        'R' => format_radix_output(
            state.output,
            state.arguments,
            state.argument_index,
            parameters,
            colon_modifier,
            at_sign_modifier,
        )?,
        'T' => format_tab_output(state.output, parameters, colon_modifier, at_sign_modifier)?,
        'W' => format_write_output(
            state.output,
            state.arguments,
            state.argument_index,
            parameters,
        )?,
        '}' => {
            return Err(RuntimeError::InvalidForm {
                message: "unexpected format iteration terminator ~}".to_string(),
                span: None,
            });
        }
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: format!("unsupported format directive ~{directive}"),
                span: None,
            });
        }
    }
    Ok(None)
}

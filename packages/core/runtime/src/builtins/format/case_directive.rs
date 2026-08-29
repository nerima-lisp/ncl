use super::*;

pub(super) fn format_case_directive(
    state: &mut FormatControlState<'_>,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<Option<FormatTermination>, RuntimeError> {
    if !parameters.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format case conversion does not accept parameters".to_string(),
            span: None,
        });
    }
    let body_end = format_case_conversion_end(state.characters, *state.character_index)?;
    let body = &state.characters[*state.character_index..body_end];
    *state.character_index = body_end + 2;
    let (formatted, consumed, termination) = format_control_characters(
        body,
        &state.arguments[*state.argument_index..],
        state.colon_iteration_last,
    )?;
    state.output.push_str(&format_case_conversion(
        &formatted,
        colon_modifier,
        at_sign_modifier,
    ));
    *state.argument_index += consumed;
    Ok(termination)
}

pub(super) fn format_case_conversion(
    text: &str,
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> String {
    let mut output = String::new();
    if colon_modifier && at_sign_modifier {
        for character in text.chars() {
            output.extend(character.to_uppercase());
        }
        return output;
    }

    if colon_modifier {
        let mut word_start = true;
        for character in text.chars() {
            if character.is_alphanumeric() {
                if word_start {
                    output.extend(character.to_uppercase());
                } else {
                    output.extend(character.to_lowercase());
                }
                word_start = false;
            } else {
                output.push(character);
                word_start = true;
            }
        }
        return output;
    }

    if at_sign_modifier {
        let mut first_word = true;
        let mut word_start = true;
        for character in text.chars() {
            if first_word && character.is_whitespace() {
                first_word = false;
                output.push(character);
            } else if first_word && character.is_alphanumeric() {
                if word_start {
                    output.extend(character.to_uppercase());
                } else {
                    output.extend(character.to_lowercase());
                }
                word_start = false;
            } else {
                output.extend(character.to_lowercase());
            }
        }
        return output;
    }

    for character in text.chars() {
        output.extend(character.to_lowercase());
    }
    output
}

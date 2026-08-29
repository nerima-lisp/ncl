use crate::RuntimeError;

pub(in crate::builtins::format) fn format_directive_prefix(
    characters: &[char],
    start: usize,
) -> Result<(usize, bool, bool), RuntimeError> {
    let mut directive_index = start;
    while directive_index < characters.len() {
        match characters[directive_index] {
            ',' | '#' | 'v' | 'V' => directive_index += 1,
            '\'' => {
                directive_index += 1;
                if directive_index >= characters.len() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format character parameter is missing its character".to_string(),
                        span: None,
                    });
                }
                directive_index += 1;
            }
            '-' | '0'..='9' => {
                if characters[directive_index] == '-' {
                    directive_index += 1;
                }
                let digit_start = directive_index;
                while directive_index < characters.len()
                    && characters[directive_index].is_ascii_digit()
                {
                    directive_index += 1;
                }
                if digit_start == directive_index {
                    return Err(RuntimeError::InvalidForm {
                        message: "format numeric parameter needs digits".to_string(),
                        span: None,
                    });
                }
            }
            _ => break,
        }
    }
    let mut colon_modifier = false;
    let mut at_sign_modifier = false;
    while directive_index < characters.len() {
        match characters[directive_index] {
            ':' => {
                colon_modifier = true;
                directive_index += 1;
            }
            '@' => {
                at_sign_modifier = true;
                directive_index += 1;
            }
            _ => break,
        }
    }
    Ok((directive_index, colon_modifier, at_sign_modifier))
}

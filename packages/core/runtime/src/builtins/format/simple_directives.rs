#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn format_simple_directive(
    directive: char,
    output: &mut String,
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<bool, RuntimeError> {
    match directive {
        'P' => {
            if !parameters.is_empty() {
                return Err(RuntimeError::InvalidForm {
                    message: "format ~P does not accept parameters".to_string(),
                    span: None,
                });
            }
            let argument = if colon_modifier {
                let index =
                    argument_index
                        .checked_sub(1)
                        .ok_or_else(|| RuntimeError::InvalidForm {
                            message: "format ~:P has no previous argument".to_string(),
                            span: None,
                        })?;
                arguments
                    .get(index)
                    .ok_or_else(|| RuntimeError::InvalidForm {
                        message: "format ~:P has no previous argument".to_string(),
                        span: None,
                    })?
            } else {
                format_argument("~P", arguments, argument_index)?
            };
            let value = integer_argument("format", argument)?;
            if at_sign_modifier {
                output.push_str(if value == 1 { "y" } else { "ies" });
            } else if value != 1 {
                output.push('s');
            }
            Ok(true)
        }
        'C' => {
            let argument = format_argument("~C", arguments, argument_index)?;
            let Value::Character(character) = argument else {
                return Err(type_error("format", "a character for ~C", argument));
            };
            output.push_str(&format_character_directive(
                *character,
                colon_modifier,
                at_sign_modifier,
            ));
            Ok(true)
        }
        '%' | '&' | '|' | '~' => {
            let count = format_parameter_count(parameters, 0, 1)?;
            for repetition in 0..count {
                match directive {
                    '%' if repetition > 0 || (!output.is_empty() && !output.ends_with('\n')) => {
                        output.push('\n');
                    }
                    '&' if repetition == 0 => {
                        if !output.is_empty() && !output.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                    '&' => output.push('\n'),
                    '|' => output.push('\x0c'),
                    '~' => output.push('~'),
                    _ => {}
                }
            }
            Ok(true)
        }
        '_' => {
            if !parameters.is_empty() {
                return Err(RuntimeError::InvalidForm {
                    message: "format ~_ does not accept parameters".to_string(),
                    span: None,
                });
            }
            Ok(true)
        }
        '\n' => {
            if !parameters.is_empty() {
                return Err(RuntimeError::InvalidForm {
                    message: "format newline directive does not accept parameters".to_string(),
                    span: None,
                });
            }
            if colon_modifier || at_sign_modifier {
                output.push('\n');
            }
            Ok(true)
        }
        'I' => {
            if at_sign_modifier {
                return Err(RuntimeError::InvalidForm {
                    message: "format ~I does not support the at-sign modifier".to_string(),
                    span: None,
                });
            }
            if parameters.len() > 1 {
                return Err(RuntimeError::InvalidForm {
                    message: "format ~I accepts at most one parameter".to_string(),
                    span: None,
                });
            }
            let _ = format_parameter_count(parameters, 0, 0)?;
            Ok(true)
        }
        '*' => {
            let count = format_parameter_count(parameters, 0, 1)?;
            *argument_index = argument_index.saturating_add(count).min(arguments.len());
            Ok(true)
        }
        _ => Ok(false),
    }
}

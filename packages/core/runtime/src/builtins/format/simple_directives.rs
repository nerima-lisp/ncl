#[allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn format_simple_directive(
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
            let value = integer_value("format", argument)?;
            let one = ibig::IBig::from(1);
            if at_sign_modifier {
                output.push_str(if value == one { "y" } else { "ies" });
            } else if value != one {
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
                    '%' => output.push('\n'),
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
            if colon_modifier && at_sign_modifier {
                return Err(RuntimeError::InvalidForm {
                    message: "format ~* cannot combine the colon and at-sign modifiers".to_string(),
                    span: None,
                });
            }
            let count =
                format_parameter_count(parameters, 0, if at_sign_modifier { 0 } else { 1 })?;
            let next_index = if at_sign_modifier {
                count
            } else if colon_modifier {
                argument_index
                    .checked_sub(count)
                    .ok_or_else(|| RuntimeError::InvalidForm {
                        message: "format ~:* moved before the first argument".to_string(),
                        span: None,
                    })?
            } else {
                argument_index
                    .checked_add(count)
                    .ok_or_else(|| RuntimeError::InvalidForm {
                        message: "format ~* moved beyond the argument list".to_string(),
                        span: None,
                    })?
            };
            if next_index > arguments.len() {
                return Err(RuntimeError::InvalidForm {
                    message: "format ~* moved beyond the argument list".to_string(),
                    span: None,
                });
            }
            *argument_index = next_index;
            Ok(true)
        }
        _ => Ok(false),
    }
}

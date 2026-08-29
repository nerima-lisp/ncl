use crate::builtins::format::model::FormatParameter;
use crate::builtins::format::output::format_argument;
use crate::builtins::numbers::integer_argument;
use crate::{RuntimeError, Value};

pub(in crate::builtins::format) fn parse_format_parameters(
    characters: &[char],
    character_index: &mut usize,
    arguments: &[Value],
    argument_index: &mut usize,
) -> Result<Vec<FormatParameter>, RuntimeError> {
    let mut parameters = Vec::new();
    let mut current_parameter = None;
    let mut comma_seen = false;
    while *character_index < characters.len() {
        match characters[*character_index] {
            ',' => {
                parameters.push(current_parameter.take().unwrap_or(FormatParameter::Missing));
                comma_seen = true;
                *character_index += 1;
            }
            '\'' => {
                *character_index += 1;
                let character = characters.get(*character_index).copied().ok_or_else(|| {
                    RuntimeError::InvalidForm {
                        message: "format character parameter is missing its character".to_string(),
                        span: None,
                    }
                })?;
                current_parameter = Some(FormatParameter::Character(character));
                comma_seen = false;
                *character_index += 1;
            }
            '#' => {
                *character_index += 1;
                let remaining = i64::try_from(arguments.len().saturating_sub(*argument_index))
                    .unwrap_or(i64::MAX);
                current_parameter = Some(FormatParameter::Number(remaining));
                comma_seen = false;
            }
            'v' | 'V' => {
                *character_index += 1;
                let argument = format_argument("format parameter", arguments, argument_index)?;
                current_parameter = Some(FormatParameter::Number(integer_argument(
                    "format parameter",
                    argument,
                )?));
                comma_seen = false;
            }
            '-' | '0'..='9' => {
                let start = *character_index;
                if characters[*character_index] == '-' {
                    *character_index += 1;
                }
                let digit_start = *character_index;
                while *character_index < characters.len()
                    && characters[*character_index].is_ascii_digit()
                {
                    *character_index += 1;
                }
                if digit_start == *character_index {
                    return Err(RuntimeError::InvalidForm {
                        message: "format numeric parameter needs digits".to_string(),
                        span: None,
                    });
                }
                let text = characters[start..*character_index]
                    .iter()
                    .collect::<String>();
                let value = text.parse::<i64>().map_err(|_| RuntimeError::InvalidForm {
                    message: format!("format numeric parameter is out of range: {text}"),
                    span: None,
                })?;
                current_parameter = Some(FormatParameter::Number(value));
                comma_seen = false;
            }
            _ => break,
        }
    }
    if let Some(parameter) = current_parameter {
        parameters.push(parameter);
    } else if comma_seen {
        parameters.push(FormatParameter::Missing);
    }
    Ok(parameters)
}

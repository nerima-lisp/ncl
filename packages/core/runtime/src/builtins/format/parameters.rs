#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn format_parameter_number(
    parameters: &[FormatParameter],
    index: usize,
    default: i64,
) -> Result<i64, RuntimeError> {
    match parameters
        .get(index)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => Ok(default),
        FormatParameter::Number(value) => Ok(value),
        FormatParameter::Character(_) => Err(RuntimeError::InvalidForm {
            message: format!("format parameter {index} must be numeric"),
            span: None,
        }),
    }
}

pub(super) fn format_parameter_count(
    parameters: &[FormatParameter],
    index: usize,
    default: i64,
) -> Result<usize, RuntimeError> {
    let value = format_parameter_number(parameters, index, default)?;
    usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
        message: format!("format parameter {index} must be non-negative"),
        span: None,
    })
}

pub(super) fn format_parameter_character(
    parameters: &[FormatParameter],
    index: usize,
    default: char,
) -> Result<char, RuntimeError> {
    match parameters
        .get(index)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => Ok(default),
        FormatParameter::Character(value) => Ok(value),
        FormatParameter::Number(_) => Err(RuntimeError::InvalidForm {
            message: format!("format parameter {index} must be a character"),
            span: None,
        }),
    }
}

pub(super) fn format_iteration_limit(
    parameters: &[FormatParameter],
) -> Result<Option<usize>, RuntimeError> {
    if parameters.is_empty() || matches!(parameters[0], FormatParameter::Missing) {
        Ok(None)
    } else {
        Ok(Some(format_parameter_count(parameters, 0, 0)?))
    }
}

pub(super) fn format_text_field(
    text: &str,
    parameters: &[FormatParameter],
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let column_increment = format_parameter_count(parameters, 1, 1)?;
    let minimum_padding = format_parameter_count(parameters, 2, 0)?;
    let padding_character = format_parameter_character(parameters, 3, ' ')?;
    if column_increment == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format column increment must be positive".to_string(),
            span: None,
        });
    }
    let width = text.chars().count();
    let mut target = minimum_column.max(width.saturating_add(minimum_padding));
    if target > minimum_column {
        let remainder = (target - minimum_column) % column_increment;
        if remainder != 0 {
            target += column_increment - remainder;
        }
    }
    let padding = target.saturating_sub(width);
    let mut formatted = String::new();
    if at_sign_modifier {
        formatted.extend(std::iter::repeat_n(padding_character, padding));
        formatted.push_str(text);
    } else {
        formatted.push_str(text);
        formatted.extend(std::iter::repeat_n(padding_character, padding));
    }
    Ok(formatted)
}

#![allow(clippy::wildcard_imports)]
use super::*;

pub(crate) fn format_grouped_digits(digits: &str, separator: char, interval: usize) -> String {
    if digits.is_empty() || interval == 0 {
        return digits.to_string();
    }
    let digit_count = digits.chars().count();
    let mut grouped = String::new();
    for (index, character) in digits.chars().enumerate() {
        if index != 0 && (digit_count - index).is_multiple_of(interval) {
            grouped.push(separator);
        }
        grouped.push(character);
    }
    grouped
}

pub(crate) fn format_character_directive(
    character: char,
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> String {
    let name = match character {
        '\0' => Some("Null"),
        '\x07' => Some("Bell"),
        '\x08' => Some("Backspace"),
        '\t' => Some("Tab"),
        '\n' => Some("Newline"),
        '\x0c' => Some("Page"),
        '\r' => Some("Return"),
        ' ' => Some("Space"),
        _ => None,
    };
    if at_sign_modifier {
        let mut result = String::from("#\\");
        if let Some(name) = name {
            result.push_str(name);
        } else {
            result.push(character);
        }
        result
    } else if colon_modifier {
        name.map_or_else(|| character.to_string(), str::to_string)
    } else {
        character.to_string()
    }
}

pub(crate) fn format_radix_directive(
    value: &ibig::IBig,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if let Some(parameter) = parameters.first().copied()
        && !matches!(parameter, FormatParameter::Missing)
    {
        let radix = match parameter {
            FormatParameter::Number(value) => {
                u32::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format radix must be between 2 and 36".to_string(),
                    span: None,
                })?
            }
            FormatParameter::Missing => unreachable!(
                "the enclosing if already excluded FormatParameter::Missing via matches!"
            ),
            FormatParameter::Character(_) => {
                return Err(RuntimeError::InvalidForm {
                    message: "format radix must be numeric".to_string(),
                    span: None,
                });
            }
        };
        if !(2..=36).contains(&radix) {
            return Err(RuntimeError::InvalidForm {
                message: "format radix must be between 2 and 36".to_string(),
                span: None,
            });
        }
        return format_integer_directive(value, radix, &parameters[1..], false, at_sign_modifier);
    }
    if at_sign_modifier {
        format_roman_number(value, colon_modifier)
    } else {
        Ok(format_english_number_value(value, colon_modifier))
    }
}

pub(crate) fn format_roman_number(
    value: &ibig::IBig,
    old_style: bool,
) -> Result<String, RuntimeError> {
    let maximum = if old_style { 4999 } else { 3999 };
    if value < &ibig::IBig::from(1) || value > &ibig::IBig::from(maximum) {
        let style = if old_style { "old-style " } else { "" };
        return Err(RuntimeError::InvalidForm {
            message: format!("format {style}Roman numeral value must be between 1 and {maximum}"),
            span: None,
        });
    }

    let numerals: &[(u32, &str)] = if old_style {
        &[
            (1000_u32, "M"),
            (500, "D"),
            (100, "C"),
            (50, "L"),
            (10, "X"),
            (5, "V"),
            (1, "I"),
        ]
    } else {
        &[
            (1000_u32, "M"),
            (900, "CM"),
            (500, "D"),
            (400, "CD"),
            (100, "C"),
            (90, "XC"),
            (50, "L"),
            (40, "XL"),
            (10, "X"),
            (9, "IX"),
            (5, "V"),
            (4, "IV"),
            (1, "I"),
        ]
    };
    let mut remainder = value.clone();
    let mut result = String::new();
    for (unit, numeral) in numerals {
        let unit = ibig::IBig::from(*unit);
        while remainder >= unit {
            result.push_str(numeral);
            remainder -= &unit;
        }
    }
    Ok(result)
}

pub(crate) fn format_argument<'a>(
    directive: &str,
    arguments: &'a [Value],
    argument_index: &mut usize,
) -> Result<&'a Value, RuntimeError> {
    let argument = arguments
        .get(*argument_index)
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: format!("format directive {directive} needs another argument"),
            span: None,
        })?;
    *argument_index += 1;
    Ok(argument)
}

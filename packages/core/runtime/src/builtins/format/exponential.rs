#![allow(clippy::wildcard_imports)]

use super::*;

pub(super) fn format_non_finite_exponential(
    value: f64,
    at_sign_modifier: bool,
    minimum_column: usize,
    overflow_character: Option<char>,
    padding_character: char,
) -> String {
    let sign = if value.is_sign_negative() {
        Some('-')
    } else if at_sign_modifier {
        Some('+')
    } else {
        None
    };
    let formatted = format!(
        "{}{}",
        sign.map_or("", |sign| if sign == '-' { "-" } else { "+" }),
        if value.is_nan() { "NaN" } else { "Inf" }
    );
    apply_exponential_field(
        formatted,
        minimum_column,
        overflow_character,
        padding_character,
    )
}

pub(super) fn exponential_digit_parameters(
    parameters: &[FormatParameter],
) -> Result<(Option<usize>, Option<usize>), RuntimeError> {
    let parse = |index, kind| match parameters
        .get(index)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => Ok(None),
        FormatParameter::Number(value) => {
            usize::try_from(value)
                .map(Some)
                .map_err(|_| RuntimeError::InvalidForm {
                    message: format!("format {kind} digit count must be non-negative"),
                    span: None,
                })
        }
        FormatParameter::Character(_) => Err(RuntimeError::InvalidForm {
            message: format!("format parameter {index} must be numeric"),
            span: None,
        }),
    };
    Ok((parse(1, "fractional")?, parse(2, "exponent")?))
}

pub(super) fn apply_exponential_field(
    formatted: String,
    minimum_column: usize,
    overflow_character: Option<char>,
    padding_character: char,
) -> String {
    let width = formatted.chars().count();
    if minimum_column > 0 && width > minimum_column {
        if let Some(overflow_character) = overflow_character {
            return std::iter::repeat_n(overflow_character, minimum_column).collect();
        }
        return formatted;
    }
    let padding = minimum_column.saturating_sub(width);
    let mut result = String::new();
    result.extend(std::iter::repeat_n(padding_character, padding));
    result.push_str(&formatted);
    result
}

#[derive(Clone, Copy)]
pub(super) struct ExponentialFiniteOptions {
    pub(super) significant_digits: usize,
    pub(super) fractional_digits: usize,
    pub(super) trim_fractional_zeroes: bool,
    pub(super) scale: i32,
    pub(super) requested_exponent_digits: Option<usize>,
    pub(super) exponent_character: char,
    pub(super) at_sign_modifier: bool,
}

pub(super) fn format_exponential_finite(
    value: f64,
    options: ExponentialFiniteOptions,
) -> Result<String, RuntimeError> {
    let ExponentialFiniteOptions {
        significant_digits,
        fractional_digits,
        trim_fractional_zeroes,
        scale,
        requested_exponent_digits,
        exponent_character,
        at_sign_modifier,
    } = options;
    let scientific = format!("{:.*e}", significant_digits.saturating_sub(1), value.abs());
    let (coefficient, exponent_text) = scientific
        .split_once('e')
        .or_else(|| scientific.split_once('E'))
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: "format exponential conversion did not produce an exponent".to_string(),
            span: None,
        })?;
    let raw_exponent = exponent_text
        .parse::<i32>()
        .map_err(|_| RuntimeError::InvalidForm {
            message: "format exponential conversion produced an invalid exponent".to_string(),
            span: None,
        })?;
    let mut digits = coefficient
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<Vec<_>>();
    digits.truncate(significant_digits);
    digits.resize(significant_digits, '0');
    let mut mantissa = String::new();
    match scale.cmp(&0) {
        std::cmp::Ordering::Greater => {
            let before = usize::try_from(scale).unwrap_or(usize::MAX);
            for index in 0..before {
                mantissa.push(*digits.get(index).unwrap_or(&'0'));
            }
            mantissa.push('.');
            let after = fractional_digits.saturating_sub(before.saturating_sub(1));
            for index in 0..after {
                mantissa.push(*digits.get(before + index).unwrap_or(&'0'));
            }
        }
        std::cmp::Ordering::Equal => {
            mantissa.push_str("0.");
            for index in 0..fractional_digits {
                mantissa.push(*digits.get(index).unwrap_or(&'0'));
            }
        }
        std::cmp::Ordering::Less => {
            let magnitude = usize::try_from(scale.unsigned_abs()).unwrap_or(usize::MAX);
            mantissa.push_str("0.");
            mantissa.extend(std::iter::repeat_n('0', magnitude));
            let count = fractional_digits.saturating_sub(magnitude);
            for index in 0..count {
                mantissa.push(*digits.get(index).unwrap_or(&'0'));
            }
        }
    }
    if trim_fractional_zeroes && let Some(decimal_index) = mantissa.find('.') {
        while mantissa.len() > decimal_index + 2 && mantissa.ends_with('0') {
            mantissa.pop();
        }
    }
    let exponent = i64::from(raw_exponent)
        .checked_sub(i64::from(scale) - 1)
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: "format exponent is out of range".to_string(),
            span: None,
        })?;
    let mut formatted = String::new();
    if value.is_sign_negative() {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&mantissa);
    formatted.push(exponent_character);
    formatted.push(if exponent < 0 { '-' } else { '+' });
    let magnitude = exponent.unsigned_abs().to_string();
    if let Some(width) = requested_exponent_digits {
        formatted.extend(std::iter::repeat_n(
            '0',
            width.saturating_sub(magnitude.len()),
        ));
    }
    formatted.push_str(&magnitude);
    Ok(formatted)
}

use crate::RuntimeError;

#[derive(Clone, Copy)]
pub(in crate::builtins::format) struct ExponentialFiniteOptions {
    pub(in crate::builtins::format) significant_digits: usize,
    pub(in crate::builtins::format) fractional_digits: usize,
    pub(in crate::builtins::format) trim_fractional_zeroes: bool,
    pub(in crate::builtins::format) scale: i32,
    pub(in crate::builtins::format) requested_exponent_digits: Option<usize>,
    pub(in crate::builtins::format) exponent_character: char,
    pub(in crate::builtins::format) at_sign_modifier: bool,
}

pub(in crate::builtins::format) fn format_exponential_finite(
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

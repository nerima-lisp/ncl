use super::*;

pub(super) fn format_integer_directive(
    value: i64,
    radix: u32,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let padding_character = format_parameter_character(parameters, 1, ' ')?;
    let comma_character = format_parameter_character(parameters, 2, ',')?;
    let comma_interval = format_parameter_count(parameters, 3, 3)?;
    if colon_modifier && comma_interval == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format comma interval must be positive".to_string(),
            span: None,
        });
    }

    let mut digits = format_unsigned_integer(value.unsigned_abs(), radix);
    if colon_modifier {
        digits = format_grouped_digits(&digits, comma_character, comma_interval);
    }
    let mut formatted = String::new();
    if value < 0 {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&digits);
    let padding = minimum_column.saturating_sub(formatted.chars().count());
    let mut result = String::new();
    result.extend(std::iter::repeat(padding_character).take(padding));
    result.push_str(&formatted);
    Ok(result)
}

fn format_fixed_float_digits(magnitude: f64, fractional_digits: usize) -> String {
    let mut digits = format!("{:.*}", fractional_digits, magnitude);
    let rounds_down = digits
        .parse::<f64>()
        .map_or(false, |rounded| rounded < magnitude);
    if fixed_float_is_exact_halfway(magnitude, fractional_digits) && rounds_down {
        let mut index = digits.len();
        while index > 0 {
            index -= 1;
            let byte = digits.as_bytes()[index];
            if byte == b'.' {
                continue;
            }
            if byte == b'9' {
                digits.replace_range(index..index + 1, "0");
            } else {
                let replacement = char::from(byte + 1).to_string();
                digits.replace_range(index..index + 1, &replacement);
                return digits;
            }
        }
        digits.insert(0, '1');
    }
    digits
}

fn fixed_float_is_exact_halfway(magnitude: f64, fractional_digits: usize) -> bool {
    if magnitude == 0.0 || !magnitude.is_finite() {
        return false;
    }
    let bits = magnitude.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if exponent_bits == 0 {
        (fraction, -1074)
    } else {
        ((1_u64 << 52) | fraction, exponent_bits - 1075)
    };
    if significand == 0 {
        return false;
    }
    let fractional_digits = match i64::try_from(fractional_digits) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let scaled_exponent = exponent.checked_add(fractional_digits).unwrap_or(i64::MAX);
    if scaled_exponent >= 0 {
        return false;
    }
    let denominator_power = usize::try_from(-scaled_exponent).unwrap_or(usize::MAX);
    u32::try_from(denominator_power.saturating_sub(1))
        .map(|power| significand.trailing_zeros() == power)
        .unwrap_or(false)
}

pub(super) fn format_fixed_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "unsupported format modifier before ~F".to_string(),
            span: None,
        });
    }
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let fractional_digits = match parameters
        .get(1)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format fractional digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 1 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let scale = format_parameter_number(parameters, 2, 0)?;
    let scale = i32::try_from(scale).map_err(|_| RuntimeError::InvalidForm {
        message: "format scale factor is out of range".to_string(),
        span: None,
    })?;
    let overflow_character = match parameters
        .get(3)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Character(value) => Some(value),
        FormatParameter::Number(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 3 must be a character".to_string(),
                span: None,
            });
        }
    };
    let padding_character = format_parameter_character(parameters, 4, ' ')?;
    let scaled = value * 10_f64.powi(scale);
    let negative = scaled.is_sign_negative();
    let magnitude = scaled.abs();
    let mut digits = if let Some(fractional_digits) = fractional_digits {
        let mut digits = format_fixed_float_digits(magnitude, fractional_digits);
        if fractional_digits == 0 {
            digits.push('.');
        }
        digits
    } else {
        let mut free_digits = magnitude.to_string();
        if !free_digits.contains('.') && !free_digits.contains('e') && !free_digits.contains('E') {
            free_digits.push_str(".0");
        }
        let sign_width = usize::from(negative || at_sign_modifier);
        if minimum_column == 0
            || sign_width.saturating_add(free_digits.chars().count()) <= minimum_column
        {
            free_digits
        } else {
            let integer_digits = format!("{:.0}", magnitude).chars().count().max(1);
            let max_fractional_digits =
                minimum_column.saturating_sub(sign_width.saturating_add(integer_digits));
            let mut selected_digits = None;
            for fractional_digits in (0..=max_fractional_digits).rev() {
                let mut candidate = format_fixed_float_digits(magnitude, fractional_digits);
                if fractional_digits == 0 {
                    candidate.push('.');
                } else {
                    while candidate.ends_with('0') {
                        candidate.pop();
                    }
                    if candidate.ends_with('.') {
                        candidate.push('0');
                    }
                }
                let effective_fractional_digits = candidate
                    .split_once('.')
                    .map(|(_, fractional)| fractional.chars().count())
                    .unwrap_or(0);
                if minimum_column
                    == sign_width
                        .saturating_add(effective_fractional_digits)
                        .saturating_add(1)
                    && candidate.starts_with("0.")
                {
                    candidate.remove(0);
                }
                if sign_width.saturating_add(candidate.chars().count()) <= minimum_column {
                    selected_digits = Some(candidate);
                    break;
                }
            }
            if magnitude < 1.0 && minimum_column <= sign_width.saturating_add(1) {
                if free_digits.starts_with("0.") {
                    free_digits.remove(0);
                }
                free_digits
            } else if let Some(selected_digits) = selected_digits {
                selected_digits
            } else if magnitude.fract() == 0.0 || (sign_width == 0 && minimum_column >= 2) {
                let mut zero_fraction_digits = format_fixed_float_digits(magnitude, 0);
                zero_fraction_digits.push('.');
                zero_fraction_digits
            } else {
                free_digits
            }
        }
    };
    if let Some(fractional_digits) = fractional_digits {
        if minimum_column == fractional_digits.saturating_add(1) && digits.starts_with("0.") {
            digits.remove(0);
        }
    }

    let mut formatted = String::new();
    if negative {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&digits);

    let width = formatted.chars().count();
    if minimum_column > 0 && width > minimum_column {
        if let Some(overflow_character) = overflow_character {
            return Ok(std::iter::repeat(overflow_character)
                .take(minimum_column)
                .collect());
        }
        return Ok(formatted);
    }
    let padding = minimum_column.saturating_sub(width);
    let mut result = String::new();
    result.extend(std::iter::repeat(padding_character).take(padding));
    result.push_str(&formatted);
    Ok(result)
}

pub(super) fn format_general_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "unsupported format modifier before ~G".to_string(),
            span: None,
        });
    }

    let parameter_at = |index| {
        parameters
            .get(index)
            .copied()
            .unwrap_or(FormatParameter::Missing)
    };
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let requested_fractional_digits = match parameter_at(1) {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format fractional digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 1 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let (exponent_padding, fixed_width_adjustment) = match parameter_at(2) {
        FormatParameter::Missing => (4, 0),
        FormatParameter::Number(value) => {
            let exponent_field_count =
                value
                    .checked_add(2)
                    .map(|value| value.max(0))
                    .ok_or_else(|| RuntimeError::InvalidForm {
                        message: "format exponent field count is out of range".to_string(),
                        span: None,
                    })?;
            let fixed_width_adjustment = if value < -2 {
                value
                    .checked_neg()
                    .and_then(|value| value.checked_sub(2))
                    .ok_or_else(|| RuntimeError::InvalidForm {
                        message: "format exponent field count is out of range".to_string(),
                        span: None,
                    })?
            } else {
                0
            };
            (exponent_field_count, fixed_width_adjustment)
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 2 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let exponent_character = match parameter_at(6) {
        FormatParameter::Missing => FormatParameter::Character('e'),
        parameter => parameter,
    };

    if !value.is_finite() {
        let exponential_parameters = vec![
            FormatParameter::Number(i64::try_from(minimum_column).map_err(|_| {
                RuntimeError::InvalidForm {
                    message: "format field width is out of range".to_string(),
                    span: None,
                }
            })?),
            FormatParameter::Missing,
            FormatParameter::Missing,
            parameter_at(3),
            parameter_at(4),
            parameter_at(5),
            exponent_character,
        ];
        return format_exponential_float_directive(
            value,
            &exponential_parameters,
            false,
            at_sign_modifier,
        );
    }

    let exponent = general_float_decimal_exponent(value);
    let fractional_digits = requested_fractional_digits.unwrap_or_else(|| {
        let q = general_float_default_fractional_digits(value, exponent);
        let minimum = usize::try_from(exponent.min(7).max(0)).unwrap_or(0);
        q.max(minimum).max(1)
    });
    let fixed_point =
        exponent >= 0 && fractional_digits >= usize::try_from(exponent).unwrap_or(usize::MAX);
    let fractional_digits =
        i64::try_from(fractional_digits).map_err(|_| RuntimeError::InvalidForm {
            message: "format fractional digit count is out of range".to_string(),
            span: None,
        })?;
    let minimum_column = i64::try_from(minimum_column).map_err(|_| RuntimeError::InvalidForm {
        message: "format field width is out of range".to_string(),
        span: None,
    })?;

    if fixed_point {
        let exponent_as_usize = usize::try_from(exponent).unwrap_or(0);
        let fixed_fractional_digits = fractional_digits
            .checked_sub(i64::try_from(exponent_as_usize).unwrap_or(i64::MAX))
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "format fractional digit count is out of range".to_string(),
                span: None,
            })?;
        let fixed_width = minimum_column
            .saturating_sub(exponent_padding)
            .saturating_add(fixed_width_adjustment)
            .max(0);
        let fixed_parameters = vec![
            FormatParameter::Number(fixed_width),
            FormatParameter::Number(fixed_fractional_digits),
            FormatParameter::Missing,
            parameter_at(4),
            parameter_at(5),
        ];
        let mut formatted =
            format_fixed_float_directive(value, &fixed_parameters, false, at_sign_modifier)?;
        formatted
            .extend(std::iter::repeat(' ').take(usize::try_from(exponent_padding).unwrap_or(0)));
        return Ok(formatted);
    }

    let exponential_parameters = vec![
        FormatParameter::Number(minimum_column),
        FormatParameter::Number(fractional_digits),
        parameter_at(2),
        parameter_at(3),
        parameter_at(4),
        parameter_at(5),
        exponent_character,
    ];
    format_exponential_float_directive(value, &exponential_parameters, false, at_sign_modifier)
}

fn general_float_decimal_exponent(value: f64) -> i64 {
    if value == 0.0 {
        return 1;
    }
    let magnitude = value.abs();
    let mut exponent = magnitude.log10().floor() as i64 + 1;
    while magnitude < 10_f64.powi((exponent - 1) as i32) {
        exponent -= 1;
    }
    while magnitude >= 10_f64.powi(exponent as i32) {
        exponent += 1;
    }
    exponent
}

fn general_float_default_fractional_digits(value: f64, exponent: i64) -> usize {
    let decimal = value.abs().to_string();
    let mantissa = decimal
        .split_once('e')
        .or_else(|| decimal.split_once('E'))
        .map(|(mantissa, _)| mantissa)
        .unwrap_or(&decimal);
    let mut found_nonzero = false;
    let mut significant_digits = 0usize;
    for character in mantissa.chars() {
        if !character.is_ascii_digit() {
            continue;
        }
        if character != '0' || found_nonzero {
            found_nonzero = true;
            significant_digits = significant_digits.saturating_add(1);
        }
    }
    let significant_digits = significant_digits.max(1);
    let leading_decimal_places = if exponent < 0 {
        usize::try_from(exponent.unsigned_abs()).unwrap_or(usize::MAX)
    } else {
        0
    };
    significant_digits.saturating_add(leading_decimal_places)
}

pub(super) fn format_dollar_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let fractional_digits = format_parameter_count(parameters, 0, 2)?;
    let minimum_integer_digits = format_parameter_count(parameters, 1, 1)?;
    let minimum_column = format_parameter_count(parameters, 2, 0)?;
    let padding_character = format_parameter_character(parameters, 3, ' ')?;

    let negative = value.is_sign_negative();
    let magnitude = value.abs();
    let mut digits = format!("{:.*}", fractional_digits, magnitude);
    if fractional_digits == 0 {
        digits.push('.');
    }
    let (integer_part, fractional_part) =
        digits
            .split_once('.')
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "format ~$ could not produce a fixed-point number".to_string(),
                span: None,
            })?;

    let mut numeric = String::new();
    numeric.extend(
        std::iter::repeat('0')
            .take(minimum_integer_digits.saturating_sub(integer_part.chars().count())),
    );
    numeric.push_str(integer_part);
    numeric.push('.');
    numeric.push_str(fractional_part);

    let sign = if negative {
        Some('-')
    } else if at_sign_modifier {
        Some('+')
    } else {
        None
    };
    let sign_width = usize::from(sign.is_some());
    let padding = minimum_column.saturating_sub(sign_width + numeric.chars().count());
    let mut result = String::new();
    if colon_modifier {
        if let Some(sign) = sign {
            result.push(sign);
        }
        result.extend(std::iter::repeat(padding_character).take(padding));
    } else {
        result.extend(std::iter::repeat(padding_character).take(padding));
        if let Some(sign) = sign {
            result.push(sign);
        }
    }
    result.push_str(&numeric);
    Ok(result)
}

pub(super) fn format_exponential_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "unsupported format modifier before ~E".to_string(),
            span: None,
        });
    }
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let requested_fractional_digits = match parameters
        .get(1)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format fractional digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 1 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let requested_exponent_digits = match parameters
        .get(2)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => Some(if value < 0 {
            0
        } else {
            usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                message: "format exponent digit count is out of range".to_string(),
                span: None,
            })?
        }),
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 2 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let scale = i32::try_from(format_parameter_number(parameters, 3, 1)?).map_err(|_| {
        RuntimeError::InvalidForm {
            message: "format scale factor is out of range".to_string(),
            span: None,
        }
    })?;
    if let Some(fractional_digits) = requested_fractional_digits {
        let invalid_positive_scale =
            scale > 0 && (scale as usize) >= fractional_digits.saturating_add(2);
        let invalid_negative_scale =
            scale < 0 && (scale.unsigned_abs() as usize) >= fractional_digits;
        if invalid_positive_scale || invalid_negative_scale {
            return Err(RuntimeError::InvalidForm {
                message: "format scale factor is incompatible with fractional digit count"
                    .to_string(),
                span: None,
            });
        }
    }
    let fractional_digits = requested_fractional_digits.unwrap_or_else(|| {
        let minimum = if scale > 0 {
            (scale as usize).saturating_sub(1)
        } else if scale < 0 {
            (scale.unsigned_abs() as usize).saturating_add(1)
        } else {
            0
        };
        6.max(minimum)
    });
    let significant_digits = if scale > 0 {
        fractional_digits.checked_add(1)
    } else if scale == 0 {
        Some(fractional_digits.max(1))
    } else {
        fractional_digits.checked_sub(scale.unsigned_abs() as usize)
    }
    .filter(|value| *value > 0)
    .ok_or_else(|| RuntimeError::InvalidForm {
        message: "format scale factor leaves no significant digits".to_string(),
        span: None,
    })?;
    let overflow_character = match parameters
        .get(4)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Character(value) => Some(value),
        FormatParameter::Number(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 4 must be a character".to_string(),
                span: None,
            });
        }
    };
    let padding_character = format_parameter_character(parameters, 5, ' ')?;
    let exponent_character = format_parameter_character(parameters, 6, 'e')?;
    let apply_field = |formatted: String| {
        let width = formatted.chars().count();
        if minimum_column > 0 && width > minimum_column {
            if let Some(overflow_character) = overflow_character {
                return Ok(std::iter::repeat(overflow_character)
                    .take(minimum_column)
                    .collect());
            }
            return Ok(formatted);
        }
        let padding = minimum_column.saturating_sub(width);
        let mut result = String::new();
        result.extend(std::iter::repeat(padding_character).take(padding));
        result.push_str(&formatted);
        Ok(result)
    };

    if !value.is_finite() {
        let mut formatted = String::new();
        if value.is_sign_negative() {
            formatted.push('-');
        } else if at_sign_modifier {
            formatted.push('+');
        }
        formatted.push_str(if value.is_nan() { "NaN" } else { "Inf" });
        return apply_field(formatted);
    }

    let magnitude = value.abs();
    let scientific = format!("{:.*e}", significant_digits.saturating_sub(1), magnitude);
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
        .filter(|character| character.is_ascii_digit())
        .collect::<Vec<_>>();
    digits.truncate(significant_digits);
    digits.resize(significant_digits, '0');

    let mut mantissa = String::new();
    if scale > 0 {
        let digits_before_decimal = scale as usize;
        for index in 0..digits_before_decimal {
            mantissa.push(*digits.get(index).unwrap_or(&'0'));
        }
        mantissa.push('.');
        let digits_after_decimal =
            fractional_digits.saturating_sub(digits_before_decimal.saturating_sub(1));
        for index in 0..digits_after_decimal {
            mantissa.push(*digits.get(digits_before_decimal + index).unwrap_or(&'0'));
        }
    } else if scale == 0 {
        mantissa.push_str("0.");
        for index in 0..fractional_digits {
            mantissa.push(*digits.get(index).unwrap_or(&'0'));
        }
    } else {
        mantissa.push_str("0.");
        mantissa.extend(std::iter::repeat('0').take(scale.unsigned_abs() as usize));
        let significant_fractional_digits =
            fractional_digits.saturating_sub(scale.unsigned_abs() as usize);
        for index in 0..significant_fractional_digits {
            mantissa.push(*digits.get(index).unwrap_or(&'0'));
        }
    }
    if requested_fractional_digits.is_none() {
        if let Some(decimal_index) = mantissa.find('.') {
            while mantissa.len() > decimal_index + 2 && mantissa.ends_with('0') {
                mantissa.pop();
            }
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
    if exponent < 0 {
        formatted.push('-');
    } else {
        formatted.push('+');
    }
    let exponent_magnitude = exponent.unsigned_abs().to_string();
    if let Some(exponent_width) = requested_exponent_digits {
        formatted.extend(
            std::iter::repeat('0')
                .take(exponent_width.saturating_sub(exponent_magnitude.chars().count())),
        );
    }
    formatted.push_str(&exponent_magnitude);
    apply_field(formatted)
}

fn format_grouped_digits(digits: &str, separator: char, interval: usize) -> String {
    if digits.is_empty() || interval == 0 {
        return digits.to_string();
    }
    let mut grouped = String::new();
    for (index, character) in digits.chars().enumerate() {
        if index != 0 && (digits.chars().count() - index) % interval == 0 {
            grouped.push(separator);
        }
        grouped.push(character);
    }
    grouped
}

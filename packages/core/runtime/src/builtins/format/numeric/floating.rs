fn format_fixed_float_directive(
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
        let mut digits = format!("{:.*}", fractional_digits, magnitude);
        if fractional_digits == 0 {
            digits.push('.');
        }
        digits
    } else {
        let mut digits = magnitude.to_string();
        if !digits.contains('.') && !digits.contains('e') && !digits.contains('E') {
            digits.push_str(".0");
        }
        digits
    };
    if let Some(fractional_digits) = fractional_digits
        && minimum_column == fractional_digits.saturating_add(1)
        && digits.starts_with("0.")
    {
        digits.remove(0);
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
            return Ok(std::iter::repeat_n(overflow_character, minimum_column).collect());
        }
        return Ok(formatted);
    }
    let padding = minimum_column.saturating_sub(width);
    let mut result = String::new();
    result.extend(std::iter::repeat_n(padding_character, padding));
    result.push_str(&formatted);
    Ok(result)
}

fn format_general_float_directive(
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
    let exponent_padding = match parameter_at(2) {
        FormatParameter::Missing => 4,
        FormatParameter::Number(value) => usize::try_from(value)
            .map_err(|_| RuntimeError::InvalidForm {
                message: "format exponent field count must be non-negative".to_string(),
                span: None,
            })?
            .checked_add(2)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "format exponent field count is out of range".to_string(),
                span: None,
            })?,
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
        let minimum = usize::try_from(exponent.clamp(0, 7)).unwrap_or(0);
        q.max(minimum).max(1)
    });
    let fixed_point =
        exponent >= 0 && fractional_digits >= usize::try_from(exponent).unwrap_or(usize::MAX);
    let fractional_digits =
        i64::try_from(fractional_digits).map_err(|_| RuntimeError::InvalidForm {
            message: "format fractional digit count is out of range".to_string(),
            span: None,
        })?;
    let exponent_padding =
        i64::try_from(exponent_padding).map_err(|_| RuntimeError::InvalidForm {
            message: "format exponent field count is out of range".to_string(),
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
        let fixed_width = minimum_column.saturating_sub(exponent_padding).max(0);
        let fixed_parameters = vec![
            FormatParameter::Number(fixed_width),
            FormatParameter::Number(fixed_fractional_digits),
            FormatParameter::Missing,
            parameter_at(4),
            parameter_at(5),
        ];
        let mut formatted =
            format_fixed_float_directive(value, &fixed_parameters, false, at_sign_modifier)?;
        formatted.extend(std::iter::repeat_n(
            ' ',
            usize::try_from(exponent_padding).unwrap_or(0),
        ));
        return Ok(formatted);
    }

    let exponential_parameters = vec![
        FormatParameter::Number(minimum_column),
        FormatParameter::Number(fractional_digits),
        FormatParameter::Missing,
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

fn format_dollar_float_directive(
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
    numeric.extend(std::iter::repeat_n(
        '0',
        minimum_integer_digits.saturating_sub(integer_part.chars().count()),
    ));
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
        result.extend(std::iter::repeat_n(padding_character, padding));
    } else {
        result.extend(std::iter::repeat_n(padding_character, padding));
        if let Some(sign) = sign {
            result.push(sign);
        }
    }
    result.push_str(&numeric);
    Ok(result)
}

fn format_exponential_float_directive(
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
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format exponent digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
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
    let exponent_character = format_parameter_character(parameters, 6, 'E')?;
    let apply_field = |formatted: String| {
        let width = formatted.chars().count();
        if minimum_column > 0 && width > minimum_column {
            if let Some(overflow_character) = overflow_character {
                return Ok(std::iter::repeat_n(overflow_character, minimum_column).collect());
            }
            return Ok(formatted);
        }
        let padding = minimum_column.saturating_sub(width);
        let mut result = String::new();
        result.extend(std::iter::repeat_n(padding_character, padding));
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
        mantissa.extend(std::iter::repeat_n('0', scale.unsigned_abs() as usize));
        let significant_fractional_digits =
            fractional_digits.saturating_sub(scale.unsigned_abs() as usize);
        for index in 0..significant_fractional_digits {
            mantissa.push(*digits.get(index).unwrap_or(&'0'));
        }
    }
    if requested_fractional_digits.is_none()
        && let Some(decimal_index) = mantissa.find('.')
    {
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
    if exponent < 0 {
        formatted.push('-');
    } else {
        formatted.push('+');
    }
    let exponent_magnitude = exponent.unsigned_abs().to_string();
    if let Some(exponent_width) = requested_exponent_digits {
        formatted.extend(std::iter::repeat_n(
            '0',
            exponent_width.saturating_sub(exponent_magnitude.chars().count()),
        ));
    }
    formatted.push_str(&exponent_magnitude);
    apply_field(formatted)
}

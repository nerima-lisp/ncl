#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn format_numeric_directive(
    directive: char,
    arguments: &[Value],
    argument_index: &mut usize,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if matches!(directive, 'D' | 'B' | 'O' | 'X') {
        let argument = format_argument("format integer directive", arguments, argument_index)?;
        let integer = big_integer_argument("format", argument)?;
        let radix = match directive {
            'D' => 10,
            'B' => 2,
            'O' => 8,
            'X' => 16,
            _ => unreachable!("directive was matched against D|B|O|X above"),
        };
        return format_big_integer_directive(
            &integer,
            radix,
            parameters,
            colon_modifier,
            at_sign_modifier,
        );
    }

    let argument = format_argument("format number directive", arguments, argument_index)?;
    let value = number_argument("format", argument)?.as_float();
    match directive {
        'F' => format_fixed_float_directive(value, parameters, colon_modifier, at_sign_modifier),
        'G' => format_general_float_directive(value, parameters, colon_modifier, at_sign_modifier),
        'E' => {
            format_exponential_float_directive(value, parameters, colon_modifier, at_sign_modifier)
        }
        '$' => format_dollar_float_directive(value, parameters, colon_modifier, at_sign_modifier),
        _ => unreachable!("directive is F|G|E|$ since D|B|O|X returned above"),
    }
}

pub(super) fn format_integer_directive(
    value: i64,
    radix: u32,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    format_big_integer_directive(&ibig::IBig::from(value), radix, parameters, colon_modifier, at_sign_modifier)
}

pub(super) fn format_big_integer_directive(
    value: &ibig::IBig,
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

    let formatted_value = format_big_integer_radix(value, radix);
    let negative = formatted_value.starts_with('-');
    let mut digits = if negative {
        formatted_value[1..].to_string()
    } else {
        formatted_value
    };
    if colon_modifier {
        digits = format_grouped_digits(&digits, comma_character, comma_interval);
    }
    let mut formatted = String::new();
    if negative {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&digits);
    let padding = minimum_column.saturating_sub(formatted.chars().count());
    let mut result = String::new();
    result.extend(std::iter::repeat_n(padding_character, padding));
    result.push_str(&formatted);
    Ok(result)
}

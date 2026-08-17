fn format_integer_directive(
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
    result.extend(std::iter::repeat_n(padding_character, padding));
    result.push_str(&formatted);
    Ok(result)
}

fn format_grouped_digits(digits: &str, separator: char, interval: usize) -> String {
    if digits.is_empty() || interval == 0 {
        return digits.to_string();
    }
    let mut grouped = String::new();
    for (index, character) in digits.chars().enumerate() {
        if index != 0 && (digits.chars().count() - index).is_multiple_of(interval) {
            grouped.push(separator);
        }
        grouped.push(character);
    }
    grouped
}

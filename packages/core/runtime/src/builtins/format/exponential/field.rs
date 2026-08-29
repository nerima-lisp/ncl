pub(in crate::builtins::format) fn format_non_finite_exponential(
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

pub(in crate::builtins::format) fn apply_exponential_field(
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

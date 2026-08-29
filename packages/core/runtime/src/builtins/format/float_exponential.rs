use super::*;

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
    let (requested_fractional_digits, requested_exponent_digits) =
        exponential_digit_parameters(parameters)?;
    let scale = i32::try_from(format_parameter_number(parameters, 3, 1)?).map_err(|_| {
        RuntimeError::InvalidForm {
            message: "format scale factor is out of range".to_string(),
            span: None,
        }
    })?;
    if let Some(fractional_digits) = requested_fractional_digits {
        let invalid_positive_scale = scale > 0
            && usize::try_from(scale)
                .is_ok_and(|scale| scale >= fractional_digits.saturating_add(2));
        let invalid_negative_scale = scale < 0
            && usize::try_from(scale.unsigned_abs()).is_ok_and(|scale| scale >= fractional_digits);
        if invalid_positive_scale || invalid_negative_scale {
            return Err(RuntimeError::InvalidForm {
                message: "format scale factor is incompatible with fractional digit count"
                    .to_string(),
                span: None,
            });
        }
    }
    let fractional_digits = requested_fractional_digits.unwrap_or_else(|| {
        let minimum = match scale.cmp(&0) {
            std::cmp::Ordering::Greater => usize::try_from(scale)
                .unwrap_or(usize::MAX)
                .saturating_sub(1),
            std::cmp::Ordering::Less => usize::try_from(scale.unsigned_abs())
                .unwrap_or(usize::MAX)
                .saturating_add(1),
            std::cmp::Ordering::Equal => 0,
        };
        6.max(minimum)
    });
    let significant_digits = match scale.cmp(&0) {
        std::cmp::Ordering::Greater => fractional_digits.checked_add(1),
        std::cmp::Ordering::Equal => Some(fractional_digits.max(1)),
        std::cmp::Ordering::Less => fractional_digits
            .checked_sub(usize::try_from(scale.unsigned_abs()).unwrap_or(usize::MAX)),
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
    if !value.is_finite() {
        return Ok(format_non_finite_exponential(
            value,
            at_sign_modifier,
            minimum_column,
            overflow_character,
            padding_character,
        ));
    }
    let formatted = format_exponential_finite(
        value,
        ExponentialFiniteOptions {
            significant_digits,
            fractional_digits,
            trim_fractional_zeroes: requested_fractional_digits.is_none(),
            scale,
            requested_exponent_digits,
            exponent_character,
            at_sign_modifier,
        },
    )?;
    Ok(apply_exponential_field(
        formatted,
        minimum_column,
        overflow_character,
        padding_character,
    ))
}

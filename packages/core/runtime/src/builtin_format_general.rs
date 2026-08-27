#![allow(clippy::redundant_pub_crate)]
#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) struct GeneralFloatParameters {
    minimum_column: usize,
    requested_fractional_digits: Option<usize>,
    exponent_padding: usize,
    exponent_character: FormatParameter,
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

    let parsed = parse_general_float_parameters(parameters)?;
    let parameter_at = |index| {
        parameters
            .get(index)
            .copied()
            .unwrap_or(FormatParameter::Missing)
    };
    let GeneralFloatParameters {
        minimum_column,
        requested_fractional_digits,
        exponent_padding,
        exponent_character,
    } = parsed;

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

pub(super) fn parse_general_float_parameters(
    parameters: &[FormatParameter],
) -> Result<GeneralFloatParameters, RuntimeError> {
    let parameter_at = |index| {
        parameters
            .get(index)
            .copied()
            .unwrap_or(FormatParameter::Missing)
    };
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
    Ok(GeneralFloatParameters {
        minimum_column: format_parameter_count(parameters, 0, 0)?,
        requested_fractional_digits,
        exponent_padding,
        exponent_character: match parameter_at(6) {
            FormatParameter::Missing => FormatParameter::Character('e'),
            parameter => parameter,
        },
    })
}

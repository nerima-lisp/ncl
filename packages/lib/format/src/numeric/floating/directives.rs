use super::common::{
    character_parameter, check_precision, decimal_order, default_exponential_precision,
    default_fixed_precision, exponential_core, fallback, field, fixed_core, free_float,
    integer_parameter, invalid, number_value, optional_character, repeated, scaled,
    shortest_fraction_digits, sign, signed, signed_parameter, significant_digits,
};
use crate::dispatch::FloatingDirective;
use crate::executor::{
    ExecutionState, FormatError, FormatErrorKind, FormatOutput, ResolvedParameter,
};
use crate::model::Directive;

pub(super) fn execute(
    kind: FloatingDirective,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
) -> Result<(), FormatError> {
    match kind {
        FloatingDirective::Fixed => fixed(directive, state, output, parameters),
        FloatingDirective::Exponential => exponential(directive, state, output, parameters),
        FloatingDirective::General => general(directive, state, output, parameters),
        FloatingDirective::Monetary => monetary(directive, state, output, parameters),
    }
}

fn fixed(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
) -> Result<(), FormatError> {
    if parameters.len() > 5 {
        return Err(invalid(directive));
    }
    let width = integer_parameter(parameters, 0, directive)?;
    let precision = integer_parameter(parameters, 1, directive)?;
    let scale = signed_parameter(parameters, 2, 0, directive)?;
    let overflow = optional_character(parameters, 3, directive)?;
    let pad = character_parameter(parameters, 4, ' ', directive)?;
    let word = state.take_argument(directive)?;
    let Some(value) = number_value(state, word, directive)? else {
        return fallback(directive, state, output, word, width);
    };
    let value = scaled(value, scale, directive)?;
    let text = if width.is_none() && precision.is_none() && scale == 0 {
        free_float(state, directive, word, value)?
    } else {
        let (digits, trim) = precision.map_or_else(
            || {
                (
                    width.map_or_else(
                        || shortest_fraction_digits(value),
                        |width| default_fixed_precision(value, width, directive.at_sign),
                    ),
                    true,
                )
            },
            |digits| (digits, false),
        );
        let mut text = fixed_core(value, digits, None, trim, directive)?;
        if let Some(width) = width {
            let sign_width = sign(value, directive.at_sign).len();
            if value.abs() < 1.0
                && width == sign_width + 2
                && let Some(fraction) = text.strip_prefix("0.")
            {
                text = format!(".{fraction}");
            }
            let rendered_width = sign_width.saturating_add(text.chars().count());
            if text.ends_with(".0") && rendered_width > width && rendered_width - 1 <= width {
                text.pop();
            }
        }
        text
    };
    output.write_str(&field(
        signed(&text, value, directive.at_sign),
        width,
        pad,
        overflow,
        directive,
    )?)
}

fn exponential(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
) -> Result<(), FormatError> {
    if parameters.len() > 7 {
        return Err(invalid(directive));
    }
    let width = integer_parameter(parameters, 0, directive)?;
    let precision = integer_parameter(parameters, 1, directive)?;
    let exponent_width = integer_parameter(parameters, 2, directive)?;
    let scale = signed_parameter(parameters, 3, 1, directive)?;
    let overflow = optional_character(parameters, 4, directive)?;
    let pad = character_parameter(parameters, 5, ' ', directive)?;
    let exponent_char = character_parameter(parameters, 6, 'E', directive)?;
    let word = state.take_argument(directive)?;
    let Some(value) = number_value(state, word, directive)? else {
        return fallback(directive, state, output, word, width);
    };
    let mut precision = precision;
    let digits = *precision.get_or_insert_with(|| default_exponential_precision(value, scale));
    let text = exponential_core(
        value,
        digits,
        scale,
        exponent_width,
        exponent_char,
        directive,
    )?;
    output.write_str(&field(
        signed(&text, value, directive.at_sign),
        width,
        pad,
        overflow,
        directive,
    )?)
}

fn general(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
) -> Result<(), FormatError> {
    if parameters.len() > 7 {
        return Err(invalid(directive));
    }
    let width = integer_parameter(parameters, 0, directive)?;
    let requested_digits = integer_parameter(parameters, 1, directive)?;
    let exponent_width = integer_parameter(parameters, 2, directive)?;
    let scale = signed_parameter(parameters, 3, 1, directive)?;
    let overflow = optional_character(parameters, 4, directive)?;
    let pad = character_parameter(parameters, 5, ' ', directive)?;
    let exponent_char = character_parameter(parameters, 6, 'E', directive)?;
    let word = state.take_argument(directive)?;
    let Some(value) = number_value(state, word, directive)? else {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::WrongArgumentType,
        ));
    };

    let mut requested_digits = requested_digits;
    let digits = *requested_digits.get_or_insert_with(|| {
        let minimum = usize::try_from(decimal_order(value).unwrap_or(1).min(7)).unwrap_or(0);
        significant_digits(value).max(minimum)
    });
    check_precision(digits, directive)?;
    if !value.is_finite() {
        let absolute = value.abs().to_string();
        let text = signed(&absolute, value, directive.at_sign);
        return output.write_str(&field(text, width, pad, overflow, directive)?);
    }
    let order = decimal_order(value).ok_or_else(|| invalid(directive))?;
    let n = if value == 0.0 { 1 } else { order };
    let d = i64::try_from(digits).map_err(|_| invalid(directive))?;
    let dd = d
        .checked_sub(i64::from(n))
        .ok_or_else(|| invalid(directive))?;
    let exponent_chars = exponent_width
        .unwrap_or(2)
        .checked_add(2)
        .ok_or_else(|| invalid(directive))?;

    if (0..=d).contains(&dd) {
        let fractional = usize::try_from(dd).map_err(|_| invalid(directive))?;
        let fixed_width = width.map(|width| width.saturating_sub(exponent_chars));
        let fixed = fixed_core(value, fractional, None, false, directive)?;
        let mut text = field(
            signed(&fixed, value, directive.at_sign),
            fixed_width,
            pad,
            overflow,
            directive,
        )?;
        text.push_str(&repeated(' ', exponent_chars, directive)?);
        output.write_str(&text)
    } else {
        let text = exponential_core(
            value,
            digits,
            scale,
            exponent_width,
            exponent_char,
            directive,
        )?;
        output.write_str(&field(
            signed(&text, value, directive.at_sign),
            width,
            pad,
            overflow,
            directive,
        )?)
    }
}

fn monetary(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    parameters: &[ResolvedParameter],
) -> Result<(), FormatError> {
    if parameters.len() > 4 {
        return Err(invalid(directive));
    }
    let precision = integer_parameter(parameters, 0, directive)?.unwrap_or(2);
    let minimum_integer = integer_parameter(parameters, 1, directive)?.unwrap_or(1);
    let width = integer_parameter(parameters, 2, directive)?.unwrap_or(0);
    let pad = character_parameter(parameters, 3, ' ', directive)?;
    check_precision(precision, directive)?;
    let word = state.take_argument(directive)?;
    let Some(value) = number_value(state, word, directive)? else {
        return fallback(directive, state, output, word, Some(width));
    };
    let body = fixed_core(value, precision, Some(minimum_integer), false, directive)?;
    let sign = sign(value, directive.at_sign);
    let padding = width.saturating_sub(sign.len() + body.chars().count());
    let mut text = String::new();
    if directive.colon {
        text.push_str(sign);
        text.push_str(&repeated(pad, padding, directive)?);
    } else {
        text.push_str(&repeated(pad, padding, directive)?);
        text.push_str(sign);
    }
    text.push_str(&body);
    output.write_str(&text)
}

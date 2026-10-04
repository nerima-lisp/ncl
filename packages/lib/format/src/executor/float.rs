use ncl_object::{ObjectRef, Word, classify_object, double_value};
use ncl_printer::{PrintError, PrintOptions, StringSink};

use super::parameters::{parameter_i64, parameter_usize};
use super::{ExecutionState, FormatError, next_argument};
use crate::{Directive, DirectiveKind, Parameter};

pub(super) fn execute_float_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let object = next_argument(directive, state)?;
    let ObjectRef::DoubleFloat(number) = classify_object(state.ctx, object) else {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    };
    let mut value = double_value(state.ctx, ncl_object::DoubleFloat::from_word(number))
        .map_err(PrintError::from)?;
    let scale_index = if matches!(directive.kind, DirectiveKind::E | DirectiveKind::G) {
        3
    } else {
        2
    };
    let scale = parameter_i64(directive.parameters.get(scale_index))
        .map_or_else(|| i64::from(directive.kind == DirectiveKind::E), |value| value);
    let scaled = directive.kind == DirectiveKind::F
        || directive.kind == DirectiveKind::E
        || (directive.kind == DirectiveKind::G && g_uses_exponential(value));
    if scaled {
        value *= 10_f64.powi(
            i32::try_from(scale).map_err(|_| FormatError::InvalidParameter {
                directive: directive.kind,
            })?,
        );
    }
    let mut rendered = render_float(state, value, object, directive)?;
    if directive.at_sign && !rendered.starts_with('-') {
        rendered.insert(0, '+');
    }
    let width = parameter_usize(directive.parameters.first(), directive.kind)?;
    let overflow = parameter_char(
        directive,
        if directive.kind == DirectiveKind::F {
            3
        } else {
            4
        },
    )?;
    let pad = parameter_char(
        directive,
        if directive.kind == DirectiveKind::F {
            4
        } else {
            5
        },
    )?
    .unwrap_or(' ');
    if let Some(width) = width {
        if rendered.chars().count() > width {
            if let Some(overflow) = overflow {
                rendered = std::iter::repeat_n(overflow, width).collect();
            }
        } else {
            rendered = format!(
                "{}{}",
                pad.to_string().repeat(width - rendered.chars().count()),
                rendered
            );
        }
    }
    state.sink.write_str(&rendered).map_err(FormatError::from)?;
    *state.line_start = false;
    Ok(())
}

fn g_uses_exponential(value: f64) -> bool {
    value.abs() >= 1_000_000_f64 || (value != 0.0 && value.abs() < 0.0001)
}

pub(super) fn execute_currency_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let object = next_argument(directive, state)?;
    let ObjectRef::DoubleFloat(number) = classify_object(state.ctx, object) else {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    };
    let value = double_value(state.ctx, ncl_object::DoubleFloat::from_word(number))
        .map_err(PrintError::from)?;
    let digits = parameter_usize(directive.parameters.first(), directive.kind)?.unwrap_or(2);
    let units = parameter_usize(directive.parameters.get(1), directive.kind)?.unwrap_or(1);
    let width = parameter_usize(directive.parameters.get(2), directive.kind)?.unwrap_or(0);
    let pad = parameter_char(directive, 3)?.unwrap_or(' ');
    let magnitude = format!("{value:.digits$}");
    let fallback = if directive.at_sign {
        ("+", magnitude.as_str())
    } else {
        ("", magnitude.as_str())
    };
    let (sign, unsigned) = magnitude
        .strip_prefix('-')
        .map_or(fallback, |unsigned| ("-", unsigned));
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let whole = format!("{whole:0>units$}");
    let rendered = format!("{sign}{whole}.{fraction}");
    let padding = width.saturating_sub(rendered.chars().count());
    if directive.colon {
        state.sink.write_str(sign).map_err(FormatError::from)?;
        state
            .sink
            .write_str(&format!(
                "{}{}.{fraction}",
                pad.to_string().repeat(padding),
                whole
            ))
            .map_err(FormatError::from)?;
    } else {
        state
            .sink
            .write_str(&format!("{}{rendered}", pad.to_string().repeat(padding)))
            .map_err(FormatError::from)?;
    }
    *state.line_start = false;
    Ok(())
}

fn render_float(
    state: &mut ExecutionState<'_>,
    value: f64,
    object: Word,
    directive: &Directive,
) -> Result<String, FormatError> {
    let Some(digits) = parameter_usize(directive.parameters.get(1), directive.kind)? else {
        let mut sink = StringSink::new();
        ncl_printer::write(
            state.ctx,
            state.runtime,
            object,
            &mut sink,
            &PrintOptions::new(),
        )?;
        return Ok(sink.into_string());
    };
    if directive.kind == DirectiveKind::E {
        if directive.parameters.get(1).is_none()
            && directive.parameters.get(2).is_none()
            && directive.parameters.get(3).is_none()
        {
            let mut sink = StringSink::new();
            ncl_printer::write(
                state.ctx,
                state.runtime,
                object,
                &mut sink,
                &PrintOptions::new(),
            )?;
            return Ok(sink.into_string());
        }
        Ok(render_exponential(
            value,
            digits,
            parameter_usize(directive.parameters.get(2), directive.kind)?,
            parameter_char(directive, 6)?,
        ))
    } else if directive.kind == DirectiveKind::G
        && (value.abs() >= 1_000_000_f64 || (value != 0.0 && value.abs() < 0.0001))
    {
        Ok(render_exponential(
            value,
            digits,
            parameter_usize(directive.parameters.get(2), directive.kind)?,
            parameter_char(directive, 6)?,
        ))
    } else {
        Ok(format!("{value:.digits$}"))
    }
}

#[allow(clippy::cast_possible_truncation)]
fn render_exponential(
    value: f64,
    digits: usize,
    exponent_width: Option<usize>,
    exponent_char: Option<char>,
) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let magnitude = value.abs();
    let mut exponent = if magnitude == 0.0 {
        0
    } else {
        // check-added-lines: allow(as-cast) The exponent is bounded by finite f64 values.
        magnitude.log10().floor() as i32
    };
    let mut coefficient = if magnitude == 0.0 {
        0.0
    } else {
        magnitude / 10_f64.powi(exponent)
    };
    let mut mantissa = format!("{coefficient:.digits$}");
    if coefficient >= 9.5 && mantissa.starts_with("10") {
        exponent += 1;
        coefficient /= 10.0;
        mantissa = format!("{coefficient:.digits$}");
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let exponent_sign = if exponent < 0 { "-" } else { "" };
    let exponent_digits = exponent.unsigned_abs().to_string();
    let exponent_digits = exponent_width.map_or_else(|| exponent_digits.clone(), |width| {
        format!("{exponent_digits:0>width$}")
    });
    format!(
        "{sign}{mantissa}{}{exponent_sign}{exponent_digits}",
        exponent_char.unwrap_or('e')
    )
}

fn parameter_char(directive: &Directive, index: usize) -> Result<Option<char>, FormatError> {
    match directive.parameters.get(index) {
        None | Some(Parameter::Unsupplied) => Ok(None),
        Some(Parameter::Character(value)) => Ok(Some(*value)),
        Some(Parameter::Integer(_) | Parameter::Relative | Parameter::ArgumentCount) => {
            Err(FormatError::InvalidParameter {
                directive: directive.kind,
            })
        }
    }
}

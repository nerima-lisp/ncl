use ncl_object::{ObjectRef, Word, classify_object, double_value};
use ncl_printer::{PrintError, PrintOptions, StringSink, write};

use super::parameters::{is_integer, parameter_i64, parameter_usize, parameter_width};
use super::{ExecutionState, FormatError, next_argument};
use crate::{Directive, DirectiveKind, Parameter};

pub(super) fn execute_value_kind(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let options = match directive.kind {
        DirectiveKind::A => PrintOptions::new().with_escape(false),
        DirectiveKind::S => PrintOptions::new().with_escape(true).with_readably(true),
        DirectiveKind::D => PrintOptions::new().with_base(10),
        DirectiveKind::B => PrintOptions::new().with_base(2),
        DirectiveKind::O => PrintOptions::new().with_base(8),
        DirectiveKind::X => PrintOptions::new().with_base(16),
        DirectiveKind::R => PrintOptions::new().with_base(radix_parameter(directive)?),
        DirectiveKind::W => PrintOptions::new().with_readably(true),
        DirectiveKind::F | DirectiveKind::E | DirectiveKind::G => {
            return execute_float_directive(directive, state);
        }
        DirectiveKind::Dollar => return execute_currency_directive(directive, state),
        _ => {
            // check-added-lines: allow(wildcard) non-value directives
            return Err(FormatError::InvalidParameter {
                directive: directive.kind,
            });
        }
    };
    execute_value_directive(directive, state, options)
}

fn execute_float_directive(
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
    if let Some(scale) = parameter_i64(directive.parameters.get(2)) {
        value *= 10_f64.powi(
            i32::try_from(scale).map_err(|_| FormatError::InvalidParameter {
                directive: directive.kind,
            })?,
        );
    }
    let rendered = render_float(state, value, object, directive)?;
    write_padded(state, directive, &rendered)
}

fn execute_currency_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let value = next_argument(directive, state)?;
    let ObjectRef::DoubleFloat(number) = classify_object(state.ctx, value) else {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    };
    let value = double_value(state.ctx, ncl_object::DoubleFloat::from_word(number))
        .map_err(PrintError::from)?;
    let digits = parameter_usize(directive.parameters.get(1), directive.kind)?.unwrap_or(2);
    write_padded(state, directive, &format!("{value:.digits$}"))
}

fn render_float(
    state: &mut ExecutionState<'_>,
    value: f64,
    object: Word,
    directive: &Directive,
) -> Result<String, FormatError> {
    let Some(digits) = parameter_usize(directive.parameters.get(1), directive.kind)? else {
        let mut sink = StringSink::new();
        write(
            state.ctx,
            state.runtime,
            object,
            &mut sink,
            &PrintOptions::new(),
        )?;
        return Ok(sink.into_string());
    };
    if directive.kind == DirectiveKind::E {
        Ok(format!("{value:.digits$e}"))
    } else {
        Ok(fixed_float(value, digits))
    }
}

fn fixed_float(value: f64, digits: usize) -> String {
    format!("{value:.digits$}")
}

fn write_padded(
    state: &mut ExecutionState<'_>,
    directive: &Directive,
    rendered: &str,
) -> Result<(), FormatError> {
    let width = parameter_width(directive.parameters.first(), directive.kind)?;
    let pad = directive
        .parameters
        .get(4)
        .and_then(|parameter| {
            if let Parameter::Character(value) = parameter {
                Some(*value)
            } else {
                None
            }
        })
        .unwrap_or(' ');
    let padding = width.saturating_sub(rendered.chars().count());
    if !directive.at_sign {
        for _ in 0..padding {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
    }
    state.sink.write_str(rendered).map_err(FormatError::from)?;
    if directive.at_sign {
        for _ in 0..padding {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
    }
    *state.line_start = false;
    Ok(())
}

fn execute_value_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    options: PrintOptions,
) -> Result<(), FormatError> {
    let value = next_argument(directive, state)?;
    execute_value_with_argument(directive, state, options, value)
}

fn execute_value_with_argument(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    options: PrintOptions,
    value: Word,
) -> Result<(), FormatError> {
    if matches!(
        directive.kind,
        DirectiveKind::D
            | DirectiveKind::B
            | DirectiveKind::O
            | DirectiveKind::X
            | DirectiveKind::R
    ) && !is_integer(state.ctx, value)
    {
        return Err(FormatError::NonInteger {
            directive: directive.kind,
        });
    }
    let mut rendered = StringSink::new();
    write(state.ctx, state.runtime, value, &mut rendered, &options)?;
    let rendered = rendered.into_string();
    let width = if directive.kind == DirectiveKind::R {
        0
    } else {
        parameter_width(directive.parameters.first(), directive.kind)?
    };
    let padding = width.saturating_sub(rendered.chars().count());
    if !directive.at_sign {
        for _ in 0..padding {
            state.sink.write_char(' ').map_err(FormatError::from)?;
        }
    }
    state.sink.write_str(&rendered).map_err(FormatError::from)?;
    if directive.at_sign {
        for _ in 0..padding {
            state.sink.write_char(' ').map_err(FormatError::from)?;
        }
    }
    *state.line_start = false;
    Ok(())
}

fn radix_parameter(directive: &Directive) -> Result<u32, FormatError> {
    let value = parameter_i64(directive.parameters.first()).unwrap_or(10);
    if !(2..=36).contains(&value) {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    }
    u32::try_from(value).map_err(|_| FormatError::InvalidParameter {
        directive: directive.kind,
    })
}

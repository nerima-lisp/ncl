use ncl_object::{ObjectRef, Word, classify_object};
use ncl_printer::StringSink;

use super::parameters::{parameter_i64, repeat_count_for};
use super::{Directive, DirectiveKind, EscapeScope, ExecutionState, FormatError};

pub(super) fn next_argument(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<Word, FormatError> {
    let value = state.arguments.get(*state.argument_index).copied().ok_or(
        FormatError::MissingArgument {
            directive: directive.kind,
        },
    )?;
    *state.argument_index += 1;
    Ok(value)
}

pub(super) fn next_argument_kind(
    state: &mut ExecutionState<'_>,
    directive: DirectiveKind,
) -> Result<Word, FormatError> {
    let value = state
        .arguments
        .get(*state.argument_index)
        .copied()
        .ok_or(FormatError::MissingArgument { directive })?;
    *state.argument_index += 1;
    Ok(value)
}

pub(super) fn execute_control_kind(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    match directive.kind {
        DirectiveKind::Percent | DirectiveKind::Tilde => {
            execute_repeated_control(directive, state)?;
        }
        DirectiveKind::Ampersand => {
            if !*state.line_start {
                state.sink.write_char('\n').map_err(FormatError::from)?;
            }
            state.column = 0;
            *state.line_start = true;
        }
        DirectiveKind::Bar => {
            state.sink.write_char('\u{c}').map_err(FormatError::from)?;
            state.column = 0;
            *state.line_start = true;
        }
        DirectiveKind::Underscore | DirectiveKind::I => {
            for _ in 0..repeat_count_for(directive, state)? {
                state.sink.write_char(' ').map_err(FormatError::from)?;
                state.column += 1;
            }
            *state.line_start = false;
        }
        DirectiveKind::T => {
            let count = tab_count(directive, state)?;
            for _ in 0..count {
                state.sink.write_char(' ').map_err(FormatError::from)?;
                state.column += 1;
            }
            *state.line_start = false;
        }
        DirectiveKind::P => {
            execute_plural_control(directive, state)?;
        }
        DirectiveKind::Star => {
            execute_argument_skip(directive, state)?;
        }
        DirectiveKind::UpArrow => {
            execute_up_arrow(directive, state)?;
        }
        // check-added-lines: allow(wildcard) non-control directives
        _ => {
            return Err(FormatError::InvalidParameter {
                directive: directive.kind,
            });
        }
    }
    Ok(())
}

fn execute_repeated_control(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let count = repeat_count_for(directive, state)?;
    let character = if directive.kind == DirectiveKind::Percent {
        '\n'
    } else {
        '~'
    };
    for _ in 0..count {
        state
            .sink
            .write_char(character)
            .map_err(FormatError::from)?;
    }
    if directive.kind == DirectiveKind::Percent && count > 0 {
        state.column = 0;
        *state.line_start = true;
    } else {
        state.column += count;
        *state.line_start = false;
    }
    Ok(())
}

fn execute_plural_control(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let value = if directive.colon {
        state
            .argument_index
            .checked_sub(1)
            .and_then(|index| state.arguments.get(index).copied())
            .ok_or(FormatError::MissingArgument {
                directive: directive.kind,
            })?
    } else {
        next_argument(directive, state)?
    };
    if directive.at_sign {
        state
            .sink
            .write_str(if value == Word::fixnum(1) { "y" } else { "ies" })
            .map_err(FormatError::from)?;
    } else if value != Word::fixnum(1) {
        state.sink.write_char('s').map_err(FormatError::from)?;
    }
    Ok(())
}

fn execute_argument_skip(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let offset = parameter_i64(directive.parameters.first()).unwrap_or(1);
    let current = i128::try_from(*state.argument_index).map_err(|_| FormatError::InvalidParameter {
        directive: directive.kind,
    })?;
    let target_value = if directive.at_sign {
        i128::from(offset).max(0)
    } else if directive.colon {
        (current - i128::from(offset)).max(0)
    } else {
        (current + i128::from(offset)).max(0)
    };
    let target = usize::try_from(target_value).unwrap_or(usize::MAX);
    *state.argument_index = target.min(state.arguments.len());
    Ok(())
}

fn execute_up_arrow(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let values = directive
        .parameters
        .iter()
        .map(|parameter| match parameter {
            crate::Parameter::Integer(value) => Ok(*value),
            crate::Parameter::Unsupplied | crate::Parameter::Character(_)
            | crate::Parameter::Relative
            | crate::Parameter::ArgumentCount => Err(FormatError::InvalidParameter {
                directive: directive.kind,
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let remaining = state
        .remaining_override
        .unwrap_or_else(|| state.arguments.len().saturating_sub(*state.argument_index));
    let exhausted = remaining == 0;
    let terminate = match values.as_slice() {
        [] => exhausted,
        [value] => *value == 0,
        [left, right] => left == right,
        [low, middle, high] => low <= middle && middle <= high,
        // check-added-lines: allow(wildcard) The parser accepts arbitrary parameter counts.
        _ => false,
    };
    if terminate {
        *state.argument_index = state.arguments.len();
        *state.escape = Some(if directive.colon {
            EscapeScope::All
        } else {
            EscapeScope::Current
        });
    }
    Ok(())
}

pub(super) fn execute_character(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let object = next_argument(directive, state)?;
    let ObjectRef::Character(value) = classify_object(state.ctx, object) else {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    };
    let character = char::from_u32(value).ok_or(FormatError::InvalidParameter {
        directive: directive.kind,
    })?;
    if directive.at_sign {
        state.sink.write_str("#\\").map_err(FormatError::from)?;
        let mut rendered = StringSink::new();
        ncl_printer::write(
            state.ctx,
            state.runtime,
            object,
            &mut rendered,
            &ncl_printer::PrintOptions::new().with_escape(true),
        )?;
        let rendered = rendered.into_string();
        state
            .sink
            .write_str(rendered.strip_prefix("#\\").unwrap_or(&rendered))
            .map_err(FormatError::from)?;
    } else if directive.colon {
        let mut rendered = StringSink::new();
        ncl_printer::write(
            state.ctx,
            state.runtime,
            object,
            &mut rendered,
            &ncl_printer::PrintOptions::new().with_escape(true),
        )?;
        let rendered = rendered.into_string();
        state
            .sink
            .write_str(rendered.strip_prefix("#\\").unwrap_or(&rendered))
            .map_err(FormatError::from)?;
    } else {
        state
            .sink
            .write_char(character)
            .map_err(FormatError::from)?;
    }
    *state.line_start = false;
    Ok(())
}

fn tab_count(directive: &Directive, state: &ExecutionState<'_>) -> Result<usize, FormatError> {
    let column = parameter_i64(directive.parameters.first()).unwrap_or(1);
    let increment = parameter_i64(directive.parameters.get(1)).unwrap_or(1);
    if column < 1 || increment < 0 {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    }
    let column = usize::try_from(column).map_err(|_| FormatError::InvalidParameter {
        directive: directive.kind,
    })?;
    let increment = usize::try_from(increment).map_err(|_| FormatError::InvalidParameter {
        directive: directive.kind,
    })?;
    if directive.at_sign {
        let relative = column;
        let after_relative = state.column.saturating_add(relative);
        let alignment = if increment == 0 {
            0
        } else {
            (increment - after_relative % increment) % increment
        };
        Ok(relative + alignment)
    } else if state.column < column {
        Ok(column - state.column)
    } else if increment == 0 {
        Ok(0)
    } else {
        let remainder = (state.column - column) % increment;
        Ok(increment - remainder)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use ncl_object::{Runtime, ThreadContext};

    use super::*;

    #[test]
    fn rejects_nonpositive_tab_parameters() {
        let runtime = Runtime::new().expect("runtime"); // check-added-lines: allow(panic) test setup
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register"); // check-added-lines: allow(panic) test setup
        let arguments = Vec::new();
        let mut argument_index = 0;
        let mut sink = StringSink::new();
        let mut line_start = true;
        let mut escape = None;
        let state = ExecutionState {
            arguments: &arguments,
            argument_index: &mut argument_index,
            ctx: &mut ctx,
            runtime: &runtime,
            sink: &mut sink,
            line_start: &mut line_start,
            column: 0,
            escape: &mut escape,
            remaining_override: None,
        };
        let directive = Directive {
            parameters: vec![crate::Parameter::Integer(0)],
            colon: false,
            at_sign: false,
            kind: DirectiveKind::T,
        };
        assert!(tab_count(&directive, &state).is_err()); // check-added-lines: allow(panic) test assertion
        let directive = Directive {
            parameters: vec![crate::Parameter::Integer(1), crate::Parameter::Integer(0)],
            colon: false,
            at_sign: false,
            kind: DirectiveKind::T,
        };
        assert_eq!(tab_count(&directive, &state).unwrap_or(usize::MAX), 1);
    }
}

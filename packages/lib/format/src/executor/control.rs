use ncl_object::{ObjectRef, Word, classify_object};

use super::parameters::{parameter_i64, repeat_count_for};
use super::{ExecutionState, FormatError, next_argument};
use crate::{Directive, DirectiveKind};

pub(super) fn execute_control_kind(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    match directive.kind {
        DirectiveKind::Percent | DirectiveKind::Tilde => {
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
            *state.line_start = directive.kind == DirectiveKind::Percent && count > 0;
        }
        DirectiveKind::Ampersand => {
            if !*state.line_start {
                state.sink.write_char('\n').map_err(FormatError::from)?;
            }
            *state.line_start = true;
        }
        DirectiveKind::Bar => {
            state.sink.write_char('\u{c}').map_err(FormatError::from)?;
            *state.line_start = true;
        }
        DirectiveKind::Underscore | DirectiveKind::I => {
            for _ in 0..repeat_count_for(directive, state)? {
                state.sink.write_char(' ').map_err(FormatError::from)?;
            }
            *state.line_start = false;
        }
        DirectiveKind::T => {
            let count = tab_count(directive)?;
            for _ in 0..count {
                state.sink.write_char(' ').map_err(FormatError::from)?;
            }
            *state.line_start = false;
        }
        DirectiveKind::P => {
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
        }
        DirectiveKind::Star => {
            let offset = parameter_i64(directive.parameters.first()).unwrap_or(1);
            let current = i128::try_from(*state.argument_index).map_err(|_| {
                FormatError::InvalidParameter {
                    directive: directive.kind,
                }
            })?;
            let target_value = current + i128::from(offset);
            let target_value = if target_value < 0 { 0 } else { target_value };
            let target = match usize::try_from(target_value) {
                Ok(target) => target,
                Err(_) => usize::MAX,
            };
            *state.argument_index = target.min(state.arguments.len());
        }
        DirectiveKind::UpArrow => {
            let exhausted = state.arguments.len() == *state.argument_index;
            let terminate = if directive.parameters.is_empty() {
                exhausted
            } else {
                directive
                    .parameters
                    .first()
                    .and_then(|parameter| parameter_i64(Some(parameter)))
                    .is_some_and(|limit| {
                        i64::try_from(state.arguments.len()).is_ok_and(|len| {
                            len - i64::try_from(*state.argument_index).unwrap_or(i64::MAX) <= limit
                        })
                    })
            };
            if terminate {
                *state.argument_index = state.arguments.len();
            }
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

pub(super) fn execute_character(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let value = next_argument(directive, state)?;
    let ObjectRef::Character(value) = classify_object(state.ctx, value) else {
        return Err(FormatError::InvalidParameter {
            directive: directive.kind,
        });
    };
    let character = char::from_u32(value).ok_or(FormatError::InvalidParameter {
        directive: directive.kind,
    })?;
    state
        .sink
        .write_char(character)
        .map_err(FormatError::from)?;
    *state.line_start = false;
    Ok(())
}

fn tab_count(directive: &Directive) -> Result<usize, FormatError> {
    let column = parameter_i64(directive.parameters.first()).unwrap_or(1);
    let increment = parameter_i64(directive.parameters.get(1)).unwrap_or(8);
    if column < 1 || increment < 1 {
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
    Ok(column.div_ceil(increment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonpositive_tab_parameters() {
        for parameters in [
            vec![crate::Parameter::Integer(0)],
            vec![crate::Parameter::Integer(1), crate::Parameter::Integer(0)],
        ] {
            let directive = Directive {
                parameters,
                colon: false,
                at_sign: false,
                kind: DirectiveKind::T,
            };
            assert!(tab_count(&directive).is_err());
        }
    }
}

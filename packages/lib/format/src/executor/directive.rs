use crate::{Directive, DirectiveKind};

use super::{execute_character, execute_control_kind, execute_value_kind, ExecutionState, FormatError};

pub(super) fn execute_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    match directive.kind {
        DirectiveKind::A | DirectiveKind::S | DirectiveKind::D | DirectiveKind::B
        | DirectiveKind::O | DirectiveKind::X | DirectiveKind::R | DirectiveKind::F
        | DirectiveKind::E | DirectiveKind::G | DirectiveKind::Dollar | DirectiveKind::W => {
            execute_value_kind(directive, state)?;
        }
        DirectiveKind::Percent | DirectiveKind::Ampersand | DirectiveKind::Tilde
        | DirectiveKind::Bar | DirectiveKind::Underscore | DirectiveKind::I | DirectiveKind::T
        | DirectiveKind::P | DirectiveKind::Star | DirectiveKind::UpArrow => {
            execute_control_kind(directive, state)?;
        }
        DirectiveKind::C => execute_character(directive, state)?,
        DirectiveKind::Slash => {
            if let Some(name) = directive.name.as_deref() {
                let caller = state.caller.clone().ok_or(FormatError::InvalidParameter {
                    directive: directive.kind,
                })?;
                caller.borrow_mut().call_format_function(
                    state.ctx, state.runtime, name, state.sink, state.arguments,
                    directive.colon, directive.at_sign, &directive.parameters,
                )?;
            } else if !*state.line_start {
                state.sink.write_char('\n').map_err(FormatError::from)?;
                *state.line_start = true;
            }
        }
        DirectiveKind::Greater => {
            if directive.colon && !*state.line_start {
                state.sink.write_char('\n').map_err(FormatError::from)?;
                *state.line_start = true;
            }
        }
        DirectiveKind::Newline | DirectiveKind::Less | DirectiveKind::ColonGreater
        | DirectiveKind::BraceOpen | DirectiveKind::BraceClose | DirectiveKind::Question
        | DirectiveKind::ParenOpen | DirectiveKind::ParenClose | DirectiveKind::BracketOpen
        | DirectiveKind::BracketClose | DirectiveKind::Semicolon => {}
    }
    Ok(())
}

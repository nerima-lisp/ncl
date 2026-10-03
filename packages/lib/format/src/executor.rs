//! Execution of the typed FORMAT control representation.

use ncl_object::{Runtime, ThreadContext, Word, car, cdr};
use ncl_printer::{CharSink, PrintError, StringSink};

use crate::{ControlPart, Directive, DirectiveKind, FormatControl};

mod control;
mod parameters;
mod value;

use control::{execute_character, execute_control_kind};
use parameters::{object_string, parameter_width};
use value::execute_value_kind;

/// A failure while executing a FORMAT control.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FormatError {
    /// A value-consuming directive had no corresponding argument.
    MissingArgument { directive: DirectiveKind },
    /// A numeric parameter was not valid for the requested operation.
    InvalidParameter { directive: DirectiveKind },
    /// An integer directive received a non-integer object.
    NonInteger { directive: DirectiveKind },
    /// The underlying printer or sink failed.
    Print(PrintError),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingArgument { directive } => {
                write!(formatter, "format: missing argument for ~{directive:?}")
            }
            Self::InvalidParameter { directive } => {
                write!(formatter, "format: invalid parameter for ~{directive:?}")
            }
            Self::NonInteger { directive } => {
                write!(formatter, "format: expected integer for ~{directive:?}")
            }
            Self::Print(error) => write!(formatter, "format: {error}"),
        }
    }
}

impl std::error::Error for FormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Print(error) => Some(error),
            Self::MissingArgument { .. }
            | Self::InvalidParameter { .. }
            | Self::NonInteger { .. } => None,
        }
    }
}

impl From<PrintError> for FormatError {
    fn from(value: PrintError) -> Self {
        Self::Print(value)
    }
}

/// Execute a parsed FORMAT control against `arguments` and `sink`.
///
/// Arguments are borrowed, so execution does not create replacement Lisp
/// values or alter their GC ownership.
///
/// # Errors
///
/// Returns [`FormatError`] when an argument is missing or has the wrong type,
/// a directive parameter is invalid, or the printer cannot write to the sink.
pub fn execute(
    control: &FormatControl,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sink: &mut dyn CharSink,
) -> Result<usize, FormatError> {
    let mut argument_index = 0;
    let mut line_start = true;
    let mut state = ExecutionState {
        arguments,
        argument_index: &mut argument_index,
        ctx,
        runtime,
        sink,
        line_start: &mut line_start,
    };
    execute_parts(&control.parts, 0, control.parts.len(), &mut state)?;
    Ok(argument_index)
}

fn execute_parts(
    parts: &[ControlPart],
    mut index: usize,
    end: usize,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    while index < end {
        if let Some(next) = execute_compound(parts, index, end, state)? {
            index = next;
            continue;
        }
        let Some(part) = parts.get(index) else {
            break;
        };
        match part {
            ControlPart::Literal(text) => {
                state.sink.write_str(text).map_err(FormatError::from)?;
                *state.line_start = text.ends_with('\n') || (*state.line_start && text.is_empty());
            }
            ControlPart::Directive(directive) => {
                execute_directive(directive, state)?;
                if directive.kind == DirectiveKind::UpArrow
                    && *state.argument_index >= state.arguments.len()
                {
                    break;
                }
            }
        }
        index += 1;
    }
    Ok(())
}

fn execute_compound(
    parts: &[ControlPart],
    index: usize,
    end: usize,
    state: &mut ExecutionState<'_>,
) -> Result<Option<usize>, FormatError> {
    let Some(ControlPart::Directive(directive)) = parts.get(index) else {
        return Ok(None);
    };
    let next = match directive.kind {
        DirectiveKind::BracketOpen => execute_bracket(parts, index, end, state)?,
        DirectiveKind::BraceOpen => execute_brace(parts, index, end, state)?,
        DirectiveKind::ParenOpen => execute_case_group(parts, index, end, directive, state)?,
        DirectiveKind::Less => execute_justification(parts, index, end, directive, state)?,
        DirectiveKind::Question => execute_nested(parts, index, state)?,
        DirectiveKind::A
        | DirectiveKind::S
        | DirectiveKind::C
        | DirectiveKind::R
        | DirectiveKind::D
        | DirectiveKind::B
        | DirectiveKind::O
        | DirectiveKind::X
        | DirectiveKind::F
        | DirectiveKind::E
        | DirectiveKind::G
        | DirectiveKind::Dollar
        | DirectiveKind::W
        | DirectiveKind::Underscore
        | DirectiveKind::Greater
        | DirectiveKind::ColonGreater
        | DirectiveKind::I
        | DirectiveKind::Slash
        | DirectiveKind::T
        | DirectiveKind::Star
        | DirectiveKind::BracketClose
        | DirectiveKind::BraceClose
        | DirectiveKind::ParenClose
        | DirectiveKind::P
        | DirectiveKind::Bar
        | DirectiveKind::Semicolon
        | DirectiveKind::UpArrow
        | DirectiveKind::Newline
        | DirectiveKind::Percent
        | DirectiveKind::Ampersand
        | DirectiveKind::Tilde => None,
    };
    Ok(next)
}

fn execute_bracket(
    parts: &[ControlPart],
    index: usize,
    end: usize,
    state: &mut ExecutionState<'_>,
) -> Result<Option<usize>, FormatError> {
    let close = matching(
        parts,
        index,
        end,
        DirectiveKind::BracketOpen,
        DirectiveKind::BracketClose,
    )?;
    let branches = split_branches(parts, index + 1, close);
    let selector = next_argument_kind(state, DirectiveKind::BracketOpen)?;
    let selected = selector
        .as_fixnum()
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);
    if let Some((start, branch_end)) = branches
        .get(selected)
        .copied()
        .or_else(|| branches.last().copied())
    {
        execute_parts(parts, start, branch_end, state)?;
    }
    Ok(Some(close + 1))
}

fn execute_brace(
    parts: &[ControlPart],
    index: usize,
    end: usize,
    state: &mut ExecutionState<'_>,
) -> Result<Option<usize>, FormatError> {
    let close = matching(
        parts,
        index,
        end,
        DirectiveKind::BraceOpen,
        DirectiveKind::BraceClose,
    )?;
    let mut item = next_argument_kind(state, DirectiveKind::BraceOpen)?;
    while item != Word::NIL {
        let value = car(state.ctx, item).map_err(|_| FormatError::InvalidParameter {
            directive: DirectiveKind::BraceOpen,
        })?;
        let one = [value];
        let mut nested_index = 0;
        let mut nested = ExecutionState {
            arguments: &one,
            argument_index: &mut nested_index,
            ctx: state.ctx,
            runtime: state.runtime,
            sink: state.sink,
            line_start: state.line_start,
        };
        execute_parts(parts, index + 1, close, &mut nested)?;
        item = cdr(state.ctx, item).map_err(|_| FormatError::InvalidParameter {
            directive: DirectiveKind::BraceOpen,
        })?;
    }
    Ok(Some(close + 1))
}

fn execute_case_group(
    parts: &[ControlPart],
    index: usize,
    end: usize,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<Option<usize>, FormatError> {
    let close = matching(
        parts,
        index,
        end,
        DirectiveKind::ParenOpen,
        DirectiveKind::ParenClose,
    )?;
    let mut local = StringSink::new();
    let mut nested = ExecutionState {
        arguments: state.arguments,
        argument_index: state.argument_index,
        ctx: state.ctx,
        runtime: state.runtime,
        sink: &mut local,
        line_start: state.line_start,
    };
    execute_parts(parts, index + 1, close, &mut nested)?;
    let text = local.into_string();
    let text = if directive.colon {
        text.to_lowercase()
    } else {
        text.to_uppercase()
    };
    state.sink.write_str(&text).map_err(FormatError::from)?;
    *state.line_start = text.ends_with('\n');
    Ok(Some(close + 1))
}

fn execute_justification(
    parts: &[ControlPart],
    index: usize,
    end: usize,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<Option<usize>, FormatError> {
    let close = matching(
        parts,
        index,
        end,
        DirectiveKind::Less,
        DirectiveKind::Greater,
    )?;
    let mut local = StringSink::new();
    let mut nested = ExecutionState {
        arguments: state.arguments,
        argument_index: state.argument_index,
        ctx: state.ctx,
        runtime: state.runtime,
        sink: &mut local,
        line_start: state.line_start,
    };
    execute_parts(parts, index + 1, close, &mut nested)?;
    let text = local.into_string();
    let width = if directive.kind == DirectiveKind::R {
        0
    } else {
        parameter_width(directive.parameters.first(), directive.kind)?
    };
    for _ in 0..width.saturating_sub(text.chars().count()) {
        state.sink.write_char(' ').map_err(FormatError::from)?;
    }
    state.sink.write_str(&text).map_err(FormatError::from)?;
    Ok(Some(close + 1))
}

fn execute_nested(
    _parts: &[ControlPart],
    index: usize,
    state: &mut ExecutionState<'_>,
) -> Result<Option<usize>, FormatError> {
    let control_word = next_argument_kind(state, DirectiveKind::Question)?;
    let data = next_argument_kind(state, DirectiveKind::Question)?;
    let control = object_string(state.ctx, control_word).ok_or(FormatError::InvalidParameter {
        directive: DirectiveKind::Question,
    })?;
    let nested = crate::parse(&control).map_err(|_| FormatError::InvalidParameter {
        directive: DirectiveKind::Question,
    })?;
    let mut values = Vec::new();
    let mut item = data;
    while item != Word::NIL {
        values.push(
            car(state.ctx, item).map_err(|_| FormatError::InvalidParameter {
                directive: DirectiveKind::Question,
            })?,
        );
        item = cdr(state.ctx, item).map_err(|_| FormatError::InvalidParameter {
            directive: DirectiveKind::Question,
        })?;
    }
    let mut nested_index = 0;
    let mut nested_state = ExecutionState {
        arguments: &values,
        argument_index: &mut nested_index,
        ctx: state.ctx,
        runtime: state.runtime,
        sink: state.sink,
        line_start: state.line_start,
    };
    execute_parts(&nested.parts, 0, nested.parts.len(), &mut nested_state)?;
    Ok(Some(index + 1))
}

fn matching(
    parts: &[ControlPart],
    start: usize,
    end: usize,
    opening: DirectiveKind,
    closing: DirectiveKind,
) -> Result<usize, FormatError> {
    let mut depth = 0usize;
    for (offset, part) in parts
        .iter()
        .enumerate()
        .skip(start + 1)
        .take(end.saturating_sub(start + 1))
    {
        if let ControlPart::Directive(directive) = part {
            if directive.kind == opening {
                depth += 1;
            }
            if directive.kind == closing {
                if depth == 0 {
                    return Ok(offset);
                }
                depth -= 1;
            }
        }
    }
    Err(FormatError::InvalidParameter { directive: opening })
}

fn split_branches(parts: &[ControlPart], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut branch_start = start;
    for index in start..end {
        if matches!(
            parts.get(index),
            Some(ControlPart::Directive(Directive {
                kind: DirectiveKind::Semicolon,
                ..
            }))
        ) {
            result.push((branch_start, index));
            branch_start = index + 1;
        }
    }
    result.push((branch_start, end));
    result
}

fn execute_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    match directive.kind {
        DirectiveKind::A
        | DirectiveKind::S
        | DirectiveKind::D
        | DirectiveKind::B
        | DirectiveKind::O
        | DirectiveKind::X
        | DirectiveKind::R
        | DirectiveKind::F
        | DirectiveKind::E
        | DirectiveKind::G
        | DirectiveKind::Dollar
        | DirectiveKind::W => execute_value_kind(directive, state)?,
        DirectiveKind::Percent
        | DirectiveKind::Ampersand
        | DirectiveKind::Tilde
        | DirectiveKind::Bar
        | DirectiveKind::Underscore
        | DirectiveKind::I
        | DirectiveKind::T
        | DirectiveKind::P
        | DirectiveKind::Star
        | DirectiveKind::UpArrow => execute_control_kind(directive, state)?,
        DirectiveKind::C => execute_character(directive, state)?,
        DirectiveKind::Newline | DirectiveKind::Slash => {
            if directive.kind == DirectiveKind::Slash && !*state.line_start {
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
        DirectiveKind::Less
        | DirectiveKind::ColonGreater
        | DirectiveKind::BraceOpen
        | DirectiveKind::BraceClose
        | DirectiveKind::Question
        | DirectiveKind::ParenOpen
        | DirectiveKind::ParenClose
        | DirectiveKind::BracketOpen
        | DirectiveKind::BracketClose
        | DirectiveKind::Semicolon => {}
    }
    Ok(())
}

struct ExecutionState<'a> {
    arguments: &'a [Word],
    argument_index: &'a mut usize,
    ctx: &'a mut ThreadContext,
    runtime: &'a Runtime,
    sink: &'a mut dyn CharSink,
    line_start: &'a mut bool,
}

fn next_argument(
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

fn next_argument_kind(
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

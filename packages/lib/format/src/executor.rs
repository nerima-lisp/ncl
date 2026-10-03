//! Execution of the typed FORMAT control representation.

use ncl_object::{Runtime, ThreadContext, Word, car, cdr};
use ncl_printer::{CharSink, PrintError, StringSink};

use crate::{ControlPart, Directive, DirectiveKind, FormatControl};

mod compound;
#[cfg(test)]
#[path = "executor/tests/compound_test.rs"]
mod compound_tests;
mod control;
mod parameters;
#[cfg(test)]
#[path = "executor/tests/executor_test.rs"]
mod tests;
mod value;

use compound::{execute_justification, split_branches};
use control::{execute_character, execute_control_kind, next_argument, next_argument_kind};
use parameters::{object_string, parameter_i64, parameter_usize};
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
    let arguments = arguments.to_vec();
    let mut argument_index = 0;
    let mut line_start = true;
    let mut state = ExecutionState {
        arguments: &arguments,
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
        let part = &parts[index]; // check-added-lines: allow(index) index is bounded by end
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
        DirectiveKind::BracketOpen => execute_bracket(parts, index, end, directive, state)?,
        DirectiveKind::BraceOpen => execute_brace(parts, index, end, directive, state)?,
        DirectiveKind::ParenOpen => execute_case_group(parts, index, end, directive, state)?,
        DirectiveKind::Less => execute_justification(parts, index, end, directive, state)?,
        DirectiveKind::Question => execute_nested(parts, index, directive, state)?,
        _ => None, // check-added-lines: allow(wildcard) non-compound directives
    };
    Ok(next)
}

fn execute_bracket(
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
        DirectiveKind::BracketOpen,
        DirectiveKind::BracketClose,
    )?;
    let branches = split_branches(parts, index + 1, close);
    let selected = if let Some(parameter) = directive_parameter(directive) {
        parameter
    } else if directive.colon {
        let value = next_argument_kind(state, DirectiveKind::BracketOpen)?;
        usize::from(value != Word::NIL)
    } else if directive.at_sign {
        let value = state.arguments.get(*state.argument_index).copied().ok_or(
            FormatError::MissingArgument {
                directive: DirectiveKind::BracketOpen,
            },
        )?;
        if value == Word::NIL {
            *state.argument_index += 1;
            return Ok(Some(close + 1));
        }
        0
    } else {
        let selector = next_argument_kind(state, DirectiveKind::BracketOpen)?;
        selector
            .as_fixnum()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(FormatError::InvalidParameter {
                directive: DirectiveKind::BracketOpen,
            })?
    };
    let selected_branch = branches
        .get(selected)
        .copied()
        .or_else(|| branches.iter().find(|(_, _, default)| *default).copied());
    if let Some((start, branch_end, _)) = selected_branch {
        execute_parts(parts, start, branch_end, state)?;
    }
    Ok(Some(close + 1))
}

fn directive_parameter(directive: &Directive) -> Option<usize> {
    parameter_i64(directive.parameters.first()).and_then(|value| usize::try_from(value).ok())
}

fn execute_brace(
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
        DirectiveKind::BraceOpen,
        DirectiveKind::BraceClose,
    )?;
    if directive.at_sign {
        let limit = parameter_usize(directive.parameters.first(), directive.kind)?;
        let mut repetitions = 0;
        while *state.argument_index < state.arguments.len()
            && limit.is_none_or(|limit| repetitions < limit)
        {
            let before = *state.argument_index;
            execute_parts(parts, index + 1, close, state)?;
            repetitions += 1;
            if *state.argument_index == before {
                break;
            }
        }
        return Ok(Some(close + 1));
    }
    let mut item = next_argument_kind(state, DirectiveKind::BraceOpen)?;
    let limit = parameter_usize(directive.parameters.first(), directive.kind)?;
    let mut repetitions = 0;
    while item != Word::NIL {
        let value = car(state.ctx, item).map_err(|_| FormatError::InvalidParameter {
            directive: DirectiveKind::BraceOpen,
        })?;
        let mut values = vec![value];
        if directive.colon {
            values.clear();
            let mut nested_item = value;
            while nested_item != Word::NIL {
                values.push(car(state.ctx, nested_item).map_err(|_| {
                    FormatError::InvalidParameter {
                        directive: DirectiveKind::BraceOpen,
                    }
                })?);
                nested_item =
                    cdr(state.ctx, nested_item).map_err(|_| FormatError::InvalidParameter {
                        directive: DirectiveKind::BraceOpen,
                    })?;
            }
        }
        let mut nested_index = 0;
        let mut nested = ExecutionState {
            arguments: &values,
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
        repetitions += 1;
        if limit.is_some_and(|limit| repetitions >= limit) {
            break;
        }
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
    let text = match (directive.colon, directive.at_sign) {
        (false, false) => text.to_lowercase(),
        (true, false) => capitalize_words(&text),
        (false, true) => capitalize_first_word(&text),
        (true, true) => text.to_uppercase(),
    };
    state.sink.write_str(&text).map_err(FormatError::from)?;
    *state.line_start = text.ends_with('\n');
    Ok(Some(close + 1))
}

fn capitalize_words(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut word_start = true;
    for character in text.chars() {
        if character.is_alphabetic() {
            if word_start {
                result.extend(character.to_uppercase());
                word_start = false;
            } else {
                result.extend(character.to_lowercase());
            }
        } else {
            word_start = !character.is_alphanumeric();
            result.push(character);
        }
    }
    result
}

fn capitalize_first_word(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut first = true;
    for character in text.chars() {
        if first && character.is_alphabetic() {
            result.extend(character.to_uppercase());
            first = false;
        } else if !first && character.is_alphabetic() {
            result.extend(character.to_lowercase());
        } else {
            result.push(character);
        }
    }
    result
}

fn execute_nested(
    _parts: &[ControlPart],
    index: usize,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<Option<usize>, FormatError> {
    let control_word = next_argument_kind(state, DirectiveKind::Question)?;
    let control = object_string(state.ctx, control_word).ok_or(FormatError::InvalidParameter {
        directive: DirectiveKind::Question,
    })?;
    let nested = crate::parse(&control).map_err(|_| FormatError::InvalidParameter {
        directive: DirectiveKind::Question,
    })?;
    if directive.at_sign {
        execute_parts(&nested.parts, 0, nested.parts.len(), state)?;
        return Ok(Some(index + 1));
    }
    let data = next_argument_kind(state, DirectiveKind::Question)?;
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
    arguments: &'a Vec<Word>,
    argument_index: &'a mut usize,
    ctx: &'a mut ThreadContext,
    runtime: &'a Runtime,
    sink: &'a mut dyn CharSink,
    line_start: &'a mut bool,
}

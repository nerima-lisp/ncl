//! Execution of the typed FORMAT control representation.

use std::cell::RefCell;
use std::rc::Rc;

use ncl_object::{Runtime, ThreadContext, Word, car, cdr};
use ncl_printer::{CharSink, PrintError};

use crate::{ControlPart, Directive, DirectiveKind, FormatControl, Parameter, PrettyPrinter};

mod case;
mod compound;
#[cfg(test)]
#[path = "executor/tests/compound_test.rs"]
mod compound_tests;
mod control;
mod directive;
mod float;
mod parameters;
#[cfg(test)]
#[path = "executor/tests/executor_test.rs"]
mod tests;
mod value;

use case::execute_case_group;
use compound::{execute_justification, matching, split_branches};
use control::{execute_character, execute_control_kind, next_argument, next_argument_kind};
use directive::execute_directive;
use parameters::{object_string, parameter_i64, parameter_usize, resolve_directive};
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

/// Calls a user-defined `~/name/` FORMAT function.
pub trait FormatFunctionCaller {
    /// Resolve and call `name` through the embedding runtime.
    ///
    /// # Errors
    /// Returns [`FormatError`] when the embedding cannot call the function.
    #[allow(clippy::too_many_arguments)]
    fn call_format_function(
        &mut self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
        stream: &mut dyn CharSink,
        arguments: &[Word],
        colon: bool,
        at_sign: bool,
        parameters: &[Parameter],
    ) -> Result<(), FormatError>;
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
    execute_with_options(control, arguments, ctx, runtime, None, None, sink)
}

/// Execute a FORMAT control with an embedding-provided user-function caller.
///
/// # Errors
/// Returns [`FormatError`] when a directive or callback fails.
pub fn execute_with_caller(
    control: &FormatControl,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: Option<Rc<RefCell<dyn FormatFunctionCaller>>>,
    sink: &mut dyn CharSink,
) -> Result<usize, FormatError> {
    execute_with_options(control, arguments, ctx, runtime, caller, None, sink)
}

/// Execute a FORMAT control with user-function and pretty-printer adapters.
///
/// # Errors
/// Returns [`FormatError`] when a directive, callback, or printer fails.
pub fn execute_with_options(
    control: &FormatControl,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: Option<Rc<RefCell<dyn FormatFunctionCaller>>>,
    pretty: Option<Rc<RefCell<dyn PrettyPrinter>>>,
    sink: &mut dyn CharSink,
) -> Result<usize, FormatError> {
    let arguments = arguments.to_vec();
    let mut argument_index = 0;
    let mut line_start = true;
    let mut escape = None;
    let mut state = ExecutionState {
        arguments: &arguments,
        argument_index: &mut argument_index,
        ctx,
        runtime,
        sink,
        line_start: &mut line_start,
        column: 0,
        escape: &mut escape,
        remaining_override: None,
        caller,
        pretty,
    };
    execute_parts(&control.parts, 0, control.parts.len(), &mut state)?;
    if let Some(pretty) = state.pretty.as_ref() {
        pretty.borrow_mut().finish()?;
    }
    Ok(argument_index)
}

pub fn execute_parts(
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
                if let Some((_, suffix)) = text.rsplit_once('\n') {
                    state.column = suffix.chars().count();
                } else {
                    state.column += text.chars().count();
                }
                *state.line_start = text.ends_with('\n') || (*state.line_start && text.is_empty());
            }
            ControlPart::Directive(directive) => {
                let directive = resolve_directive(directive, state)?;
                execute_directive(&directive, state)?;
                if state.escape.is_some() {
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
        DirectiveKind::BracketOpen => {
            let resolved = resolve_directive(directive, state)?;
            execute_bracket(parts, index, end, &resolved, state)?
        }
        DirectiveKind::BraceOpen => {
            let resolved = resolve_directive(directive, state)?;
            execute_brace(parts, index, end, &resolved, state)?
        }
        DirectiveKind::ParenOpen => {
            let resolved = resolve_directive(directive, state)?;
            execute_case_group(parts, index, end, &resolved, state)?
        }
        DirectiveKind::Less => {
            let resolved = resolve_directive(directive, state)?;
            execute_justification(parts, index, end, &resolved, state)?
        }
        DirectiveKind::Question => {
            let resolved = resolve_directive(directive, state)?;
            execute_nested(parts, index, &resolved, state)?
        }
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
            let mut iteration_escape = None;
            let mut nested = ExecutionState {
                arguments: state.arguments,
                argument_index: state.argument_index,
                ctx: state.ctx,
                runtime: state.runtime,
                sink: state.sink,
                line_start: state.line_start,
                column: state.column,
                escape: &mut iteration_escape,
                remaining_override: None,
                caller: state.caller.clone(),
                pretty: state.pretty.clone(),
            };
            execute_parts(parts, index + 1, close, &mut nested)?;
            repetitions += 1;
            if iteration_escape == Some(EscapeScope::All) {
                break;
            }
            if iteration_escape == Some(EscapeScope::Current) {
                continue;
            }
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
        let next_item = cdr(state.ctx, item).map_err(|_| FormatError::InvalidParameter {
            directive: DirectiveKind::BraceOpen,
        })?;
        let mut nested_index = 0;
        let mut iteration_escape = None;
        let mut nested = ExecutionState {
            arguments: &values,
            argument_index: &mut nested_index,
            ctx: state.ctx,
            runtime: state.runtime,
            sink: state.sink,
            line_start: state.line_start,
            column: state.column,
            escape: &mut iteration_escape,
            remaining_override: Some(usize::from(next_item != Word::NIL)),
            caller: state.caller.clone(),
            pretty: state.pretty.clone(),
        };
        execute_parts(parts, index + 1, close, &mut nested)?;
        if iteration_escape == Some(EscapeScope::All) {
            break;
        }
        item = next_item;
        repetitions += 1;
        if limit.is_some_and(|limit| repetitions >= limit) {
            break;
        }
    }
    Ok(Some(close + 1))
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
        column: state.column,
        escape: state.escape,
        remaining_override: state.remaining_override,
        caller: state.caller.clone(),
        pretty: state.pretty.clone(),
    };
    execute_parts(&nested.parts, 0, nested.parts.len(), &mut nested_state)?;
    Ok(Some(index + 1))
}

pub struct ExecutionState<'a> {
    pub(super) arguments: &'a Vec<Word>,
    pub(super) argument_index: &'a mut usize,
    pub(super) ctx: &'a mut ThreadContext,
    pub(super) runtime: &'a Runtime,
    pub(super) sink: &'a mut dyn CharSink,
    pub(super) line_start: &'a mut bool,
    pub(super) column: usize,
    pub(super) escape: &'a mut Option<EscapeScope>,
    pub(super) remaining_override: Option<usize>,
    pub(super) caller: Option<Rc<RefCell<dyn FormatFunctionCaller>>>,
    pub(super) pretty: Option<Rc<RefCell<dyn PrettyPrinter>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EscapeScope {
    Current,
    All,
}

//! Execution of the typed FORMAT control representation.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify_object, double_value,
    string_length, string_ref,
};
use ncl_printer::{CharSink, PrintError, PrintOptions, StringSink, write};

use crate::{ControlPart, Directive, DirectiveKind, FormatControl, Parameter};

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
        let part = &parts[index];
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
    let ControlPart::Directive(directive) = &parts[index] else {
        return Ok(None);
    };
    let next = match directive.kind {
        DirectiveKind::BracketOpen => execute_bracket(parts, index, end, state)?,
        DirectiveKind::BraceOpen => execute_brace(parts, index, end, state)?,
        DirectiveKind::ParenOpen => execute_case_group(parts, index, end, directive, state)?,
        DirectiveKind::Less => execute_justification(parts, index, end, directive, state)?,
        DirectiveKind::Question => execute_nested(parts, index, state)?,
        _ => None,
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

fn execute_value_kind(
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
    let rendered = format!("{value:.digits$}");
    write_padded(state, directive, &rendered)
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
    Ok(match directive.kind {
        DirectiveKind::E => format!("{value:.digits$e}"),
        _ => format!("{value:.digits$}"),
    })
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
        .and_then(|parameter| match parameter {
            Parameter::Character(value) => Some(*value),
            _ => None,
        })
        .unwrap_or(' ');
    if width > rendered.chars().count() && !directive.at_sign {
        for _ in 0..(width - rendered.chars().count()) {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
    }
    state.sink.write_str(rendered).map_err(FormatError::from)?;
    if width > rendered.chars().count() && directive.at_sign {
        for _ in 0..(width - rendered.chars().count()) {
            state.sink.write_char(pad).map_err(FormatError::from)?;
        }
    }
    *state.line_start = false;
    Ok(())
}

fn execute_control_kind(
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
            let value = next_argument(directive, state)?;
            if is_integer(state.ctx, value) && value != Word::fixnum(1) {
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
            let target =
                usize::try_from(target_value).map_err(|_| FormatError::InvalidParameter {
                    directive: directive.kind,
                })?;
            *state.argument_index = target.min(state.arguments.len());
        }
        DirectiveKind::UpArrow => {
            *state.argument_index = state.arguments.len();
        }
        _ => {
            return Err(FormatError::InvalidParameter {
                directive: directive.kind,
            });
        }
    }
    Ok(())
}

fn execute_character(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<(), FormatError> {
    let value = next_argument(directive, state)?;
    let character = match classify_object(state.ctx, value) {
        ObjectRef::Character(value) => char::from_u32(value),
        _ => None,
    }
    .ok_or(FormatError::InvalidParameter {
        directive: directive.kind,
    })?;
    state
        .sink
        .write_char(character)
        .map_err(FormatError::from)?;
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

struct ExecutionState<'a> {
    arguments: &'a [Word],
    argument_index: &'a mut usize,
    ctx: &'a mut ThreadContext,
    runtime: &'a Runtime,
    sink: &'a mut dyn CharSink,
    line_start: &'a mut bool,
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
    let length = rendered.chars().count();
    if width > length && !directive.at_sign {
        for _ in 0..(width - length) {
            state.sink.write_char(' ').map_err(FormatError::from)?;
        }
    }
    state.sink.write_str(&rendered).map_err(FormatError::from)?;
    if width > length && directive.at_sign {
        for _ in 0..(width - length) {
            state.sink.write_char(' ').map_err(FormatError::from)?;
        }
    }
    *state.line_start = false;
    Ok(())
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

const fn parameter_i64(parameter: Option<&Parameter>) -> Option<i64> {
    match parameter {
        Some(Parameter::Integer(value)) => Some(*value),
        _ => None,
    }
}

fn parameter_width(
    parameter: Option<&Parameter>,
    directive: DirectiveKind,
) -> Result<usize, FormatError> {
    match parameter {
        None | Some(Parameter::Unsupplied) => Ok(0),
        Some(Parameter::Integer(value)) if *value >= 0 => {
            usize::try_from(*value).map_err(|_| FormatError::InvalidParameter { directive })
        }
        Some(
            Parameter::Integer(_)
            | Parameter::Character(_)
            | Parameter::Relative
            | Parameter::ArgumentCount,
        ) => Err(FormatError::InvalidParameter { directive }),
    }
}

fn parameter_usize(
    parameter: Option<&Parameter>,
    directive: DirectiveKind,
) -> Result<Option<usize>, FormatError> {
    match parameter {
        None | Some(Parameter::Unsupplied) => Ok(None),
        Some(Parameter::Integer(value)) if *value >= 0 => usize::try_from(*value)
            .map(Some)
            .map_err(|_| FormatError::InvalidParameter { directive }),
        Some(
            Parameter::Integer(_)
            | Parameter::Character(_)
            | Parameter::Relative
            | Parameter::ArgumentCount,
        ) => Err(FormatError::InvalidParameter { directive }),
    }
}

fn object_string(ctx: &ThreadContext, value: Word) -> Option<String> {
    if !matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        return None;
    }
    let length = string_length(ctx, value).ok()?;
    (0..length)
        .map(|index| string_ref(ctx, value, index).ok())
        .collect()
}

fn repeat_count(directive: &Directive) -> Result<usize, FormatError> {
    match directive.parameters.first() {
        None | Some(Parameter::Unsupplied) => Ok(1),
        Some(Parameter::Integer(value)) if *value >= 0 => {
            usize::try_from(*value).map_err(|_| FormatError::InvalidParameter {
                directive: directive.kind,
            })
        }
        Some(Parameter::Integer(_) | Parameter::Character(_) | Parameter::Relative) => {
            Err(FormatError::InvalidParameter {
                directive: directive.kind,
            })
        }
        Some(Parameter::ArgumentCount) => Err(FormatError::InvalidParameter {
            directive: directive.kind,
        }),
    }
}

fn repeat_count_for(
    directive: &Directive,
    state: &ExecutionState<'_>,
) -> Result<usize, FormatError> {
    if matches!(directive.parameters.first(), Some(Parameter::ArgumentCount)) {
        return Ok(state.arguments.len().saturating_sub(*state.argument_index));
    }
    repeat_count(directive)
}

fn is_integer(ctx: &ThreadContext, value: Word) -> bool {
    matches!(
        classify_object(ctx, value),
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_)
    )
}

//! Minimal execution of the typed FORMAT control representation.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, classify_object};
use ncl_printer::{CharSink, PrintError, PrintOptions, write};

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
/// This deliberately implements only the value and control directives needed
/// by the first execution layer. Arguments are borrowed, so execution does not
/// create replacement Lisp values or alter their GC ownership.
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
    for part in &control.parts {
        match part {
            ControlPart::Literal(text) => {
                sink.write_str(text).map_err(FormatError::from)?;
                line_start = text.ends_with('\n') || (line_start && text.is_empty());
            }
            ControlPart::Directive(directive) => {
                execute_directive(
                    directive,
                    arguments,
                    &mut argument_index,
                    ctx,
                    runtime,
                    sink,
                    &mut line_start,
                )?;
            }
        }
    }
    Ok(argument_index)
}

fn execute_directive(
    directive: &Directive,
    arguments: &[Word],
    argument_index: &mut usize,
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sink: &mut dyn CharSink,
    line_start: &mut bool,
) -> Result<(), FormatError> {
    let mut state = ExecutionState {
        arguments,
        argument_index,
        ctx,
        runtime,
        sink,
        line_start,
    };
    match directive.kind {
        DirectiveKind::A => execute_value_directive(
            directive,
            &mut state,
            PrintOptions::new().with_escape(false),
        )?,
        DirectiveKind::S => execute_value_directive(
            directive,
            &mut state,
            PrintOptions::new().with_escape(true).with_readably(true),
        )?,
        DirectiveKind::D => {
            execute_value_directive(directive, &mut state, PrintOptions::new().with_base(10))?;
        }
        DirectiveKind::B => {
            execute_value_directive(directive, &mut state, PrintOptions::new().with_base(2))?;
        }
        DirectiveKind::O => {
            execute_value_directive(directive, &mut state, PrintOptions::new().with_base(8))?;
        }
        DirectiveKind::X => {
            execute_value_directive(directive, &mut state, PrintOptions::new().with_base(16))?;
        }
        DirectiveKind::Percent => {
            let count = repeat_count(directive)?;
            for _ in 0..count {
                state.sink.write_char('\n').map_err(FormatError::from)?;
            }
            *state.line_start = count > 0;
        }
        DirectiveKind::Ampersand => {
            if !*state.line_start {
                state.sink.write_char('\n').map_err(FormatError::from)?;
            }
            *state.line_start = true;
        }
        DirectiveKind::Tilde => {
            let count = repeat_count(directive)?;
            for _ in 0..count {
                state.sink.write_char('~').map_err(FormatError::from)?;
            }
            *state.line_start = false;
        }
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

fn execute_value_directive(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    options: PrintOptions,
) -> Result<(), FormatError> {
    let value = state.arguments.get(*state.argument_index).copied().ok_or(
        FormatError::MissingArgument {
            directive: directive.kind,
        },
    )?;
    *state.argument_index += 1;
    if matches!(
        directive.kind,
        DirectiveKind::D | DirectiveKind::B | DirectiveKind::O | DirectiveKind::X
    ) && !is_integer(state.ctx, value)
    {
        return Err(FormatError::NonInteger {
            directive: directive.kind,
        });
    }
    write(state.ctx, state.runtime, value, state.sink, &options)?;
    *state.line_start = false;
    Ok(())
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
    }
}

fn is_integer(ctx: &ThreadContext, value: Word) -> bool {
    matches!(
        classify_object(ctx, value),
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_)
    )
}

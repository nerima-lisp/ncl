//! Shared execution state and control traversal.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, bignum_limbs, bignum_sign, classify_object,
};
use ncl_printer::{CharSink, PrintError, PrintOptions, StringSink, write};

use crate::model::{ControlPart, Directive, FormatControl, Parameter};
use crate::parser::ParseErrorKind;

/// A FORMAT failure retains its location and typed cause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatError {
    pub position: usize,
    pub directive: Option<char>,
    pub kind: FormatErrorKind,
}

/// The reason a FORMAT directive could not be executed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FormatErrorKind {
    Parse(ParseErrorKind),
    MissingArgument,
    WrongArgumentType,
    InvalidParameter,
    InvalidDirectiveUse,
    ImproperList,
    Print(PrintError),
}

impl FormatError {
    pub(crate) const fn at(directive: &Directive, kind: FormatErrorKind) -> Self {
        Self {
            position: directive.position,
            directive: Some(directive.kind.character()),
            kind,
        }
    }

    pub(crate) const fn new(
        position: usize,
        directive: Option<char>,
        kind: FormatErrorKind,
    ) -> Self {
        Self {
            position,
            directive,
            kind,
        }
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "format error at offset {}", self.position)?;
        if let Some(directive) = self.directive {
            write!(formatter, " (~{directive})")?;
        }
        write!(formatter, ": {:?}", self.kind)
    }
}

impl std::error::Error for FormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            FormatErrorKind::Print(error) => Some(error),
            FormatErrorKind::Parse(_)
            | FormatErrorKind::MissingArgument
            | FormatErrorKind::WrongArgumentType
            | FormatErrorKind::InvalidParameter
            | FormatErrorKind::InvalidDirectiveUse
            | FormatErrorKind::ImproperList => None,
        }
    }
}

/// Execution may escape the current enclosing control construct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flow {
    Continue,
    Escape,
    EscapeIterations,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlFrame {
    Iteration {
        colon: bool,
        at_sign: bool,
        last_sublist: bool,
    },
    LogicalBlock,
    Justification,
}

pub struct ExecutionState<'a> {
    pub(crate) arguments: &'a [Word],
    pub(crate) index: usize,
    pub(crate) previous_is_one: Option<bool>,
    pub(crate) frames: Vec<ControlFrame>,
    pub(crate) ctx: &'a mut ThreadContext,
    pub(crate) runtime: &'a Runtime,
}

impl ExecutionState<'_> {
    pub const fn remaining(&self) -> usize {
        self.arguments.len().saturating_sub(self.index)
    }

    pub(crate) fn take_argument(&mut self, directive: &Directive) -> Result<Word, FormatError> {
        let value = self
            .arguments
            .get(self.index)
            .copied()
            .ok_or_else(|| FormatError::at(directive, FormatErrorKind::MissingArgument))?;
        self.index += 1;
        self.previous_is_one = Some(value == Word::fixnum(1));
        Ok(value)
    }

    pub(crate) fn parameters(
        &mut self,
        directive: &Directive,
    ) -> Result<Vec<Parameter>, FormatError> {
        let mut parameters = Vec::with_capacity(directive.parameters.len());
        for parameter in &directive.parameters {
            parameters.push(match parameter {
                Parameter::Relative => {
                    let value = self.take_argument(directive)?;
                    if value == Word::NIL {
                        Parameter::Unsupplied
                    } else {
                        let object = classify_object(self.ctx, value);
                        if let ObjectRef::Character(codepoint) = object {
                            let character = char::from_u32(codepoint).ok_or_else(|| {
                                FormatError::at(directive, FormatErrorKind::WrongArgumentType)
                            })?;
                            Parameter::Character(character)
                        } else if let ObjectRef::Fixnum(integer) = object {
                            Parameter::Integer(integer)
                        } else if let ObjectRef::Bignum(_) = object {
                            Parameter::Integer(integer_value(self.ctx, value).ok_or_else(|| {
                                FormatError::at(directive, FormatErrorKind::InvalidParameter)
                            })?)
                        } else {
                            return Err(FormatError::at(
                                directive,
                                FormatErrorKind::WrongArgumentType,
                            ));
                        }
                    }
                }
                Parameter::Remaining => {
                    let remaining = i64::try_from(self.remaining()).map_err(|_| {
                        FormatError::at(directive, FormatErrorKind::InvalidParameter)
                    })?;
                    Parameter::Integer(remaining)
                }
                other => *other,
            });
        }
        Ok(parameters)
    }
}

pub struct FormatOutput<'a> {
    sink: &'a mut dyn CharSink,
    pub(crate) column: usize,
    pub(crate) line_start: bool,
    pub(crate) margin: usize,
    pub(crate) indentation: usize,
    pub(crate) block_start: usize,
    pub(crate) pretty: bool,
}

impl FormatOutput<'_> {
    pub(crate) fn new(sink: &mut dyn CharSink) -> FormatOutput<'_> {
        FormatOutput {
            sink,
            column: 0,
            line_start: true,
            margin: 80,
            indentation: 0,
            block_start: 0,
            pretty: false,
        }
    }

    pub(crate) fn write_char(&mut self, character: char) -> Result<(), FormatError> {
        if self.line_start && character != '\n' {
            for _ in 0..self.indentation {
                self.sink
                    .write_char(' ')
                    .map_err(|error| FormatError::new(0, None, FormatErrorKind::Print(error)))?;
                self.column = self.column.saturating_add(1);
            }
            self.line_start = false;
        }
        self.sink
            .write_char(character)
            .map_err(|error| FormatError::new(0, None, FormatErrorKind::Print(error)))?;
        self.track(character);
        Ok(())
    }

    pub(crate) fn write_str(&mut self, text: &str) -> Result<(), FormatError> {
        for character in text.chars() {
            self.write_char(character)?;
        }
        Ok(())
    }

    pub(crate) fn print(
        &mut self,
        state: &mut ExecutionState<'_>,
        value: Word,
        options: &PrintOptions,
        directive: &Directive,
    ) -> Result<(), FormatError> {
        let rendered = Self::render(state, value, options, directive)?;
        self.write_str(&rendered).map_err(|error| {
            if matches!(&error.kind, FormatErrorKind::Print(_)) {
                FormatError::at(directive, error.kind)
            } else {
                error
            }
        })
    }

    pub(crate) fn render(
        state: &mut ExecutionState<'_>,
        value: Word,
        options: &PrintOptions,
        directive: &Directive,
    ) -> Result<String, FormatError> {
        let mut sink = StringSink::new();
        write(state.ctx, state.runtime, value, &mut sink, options)
            .map_err(|error| FormatError::at(directive, FormatErrorKind::Print(error)))?;
        Ok(sink.into_string())
    }

    const fn track(&mut self, character: char) {
        if character == '\n' {
            self.column = 0;
            self.line_start = true;
        } else {
            self.column = self.column.saturating_add(1);
            self.line_start = false;
        }
    }
}

/// Execute a parsed control string and return the number of consumed arguments.
///
/// # Errors
///
/// Returns a located, typed error when a directive cannot consume or print its
/// arguments.
pub fn execute(
    control: &FormatControl,
    arguments: &[Word],
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sink: &mut dyn CharSink,
) -> Result<usize, FormatError> {
    let mut state = ExecutionState {
        arguments,
        index: 0,
        previous_is_one: None,
        frames: Vec::new(),
        ctx,
        runtime,
    };
    let mut output = FormatOutput::new(sink);
    let _ = execute_control(&mut state, control, &mut output)?;
    Ok(state.index)
}

pub fn execute_control(
    state: &mut ExecutionState<'_>,
    control: &FormatControl,
    output: &mut FormatOutput<'_>,
) -> Result<Flow, FormatError> {
    for part in &control.parts {
        match part {
            ControlPart::Literal(text) => output.write_str(text)?,
            ControlPart::Directive(directive) => {
                let flow = crate::dispatch::execute_directive(state, directive, output)
                    .map_err(|error| locate_output_error(error, directive))?;
                if flow != Flow::Continue {
                    return Ok(flow);
                }
            }
        }
    }
    Ok(Flow::Continue)
}

fn locate_output_error(error: FormatError, directive: &Directive) -> FormatError {
    if error.directive.is_none() && matches!(&error.kind, FormatErrorKind::Print(_)) {
        FormatError::at(directive, error.kind)
    } else {
        error
    }
}

pub fn integer_value(ctx: &ThreadContext, value: Word) -> Option<i64> {
    let object = classify_object(ctx, value);
    if let ObjectRef::Fixnum(integer) = object {
        return Some(integer);
    }
    let ObjectRef::Bignum(word) = object else {
        return None;
    };
    let bignum = ncl_object::Bignum::from_word(word);
    let mut limbs = bignum_limbs(ctx, bignum).ok()?.into_iter();
    let low = limbs.next().unwrap_or_default();
    let high = limbs.next().unwrap_or_default();
    if limbs.any(|limb| limb != 0) {
        return None;
    }
    let magnitude = u64::from(low) | (u64::from(high) << 32);
    if bignum_sign(ctx, bignum).ok()? {
        if magnitude == 1_u64 << 63 {
            Some(i64::MIN)
        } else {
            i64::try_from(magnitude).ok()?.checked_neg()
        }
    } else {
        i64::try_from(magnitude).ok()
    }
}

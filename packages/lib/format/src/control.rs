use std::collections::HashSet;

use ncl_object::{
    ObjectError, ObjectRef, StringObject, ThreadContext, Word, car, cdr, classify_object,
    string_length, string_ref,
};
use ncl_printer::PrintError;

use crate::dispatch::ControlDirective;
use crate::executor::{
    ExecutionState, Flow, FormatError, FormatErrorKind, FormatOutput, ResolvedParameter,
};
use crate::model::Directive;

mod arguments;
mod case;
mod iteration;
mod recursive;
mod selection;

pub type State<'a> = ExecutionState<'a>;
pub type Output<'a> = FormatOutput<'a>;
pub type FlowResult = Result<Flow, FormatError>;

pub const fn missing_argument(directive: &Directive) -> FormatError {
    FormatError::at(directive, FormatErrorKind::MissingArgument)
}

pub const fn invalid_parameter(directive: &Directive) -> FormatError {
    FormatError::at(directive, FormatErrorKind::InvalidParameter)
}

pub const fn invalid_use(directive: &Directive) -> FormatError {
    FormatError::at(directive, FormatErrorKind::InvalidDirectiveUse)
}

pub const fn object_error(directive: &Directive, error: ObjectError) -> FormatError {
    FormatError::at(directive, FormatErrorKind::Print(PrintError::Object(error)))
}

pub fn integer_parameter(
    parameters: &[ResolvedParameter],
    index: usize,
    default: i64,
    directive: &Directive,
) -> Result<i64, FormatError> {
    match parameters.get(index) {
        None | Some(ResolvedParameter::Unsupplied) => Ok(default),
        Some(ResolvedParameter::Integer(value)) => Ok(*value),
        Some(_) => Err(invalid_parameter(directive)),
    }
}

pub fn iteration_limit(
    parameters: &[ResolvedParameter],
    directive: &Directive,
) -> Result<Option<usize>, FormatError> {
    match parameters.first() {
        None | Some(ResolvedParameter::Unsupplied) => Ok(None),
        Some(ResolvedParameter::Integer(value)) if *value >= 0 => usize::try_from(*value)
            .map(Some)
            .map_err(|_| invalid_parameter(directive)),
        Some(_) => Err(invalid_parameter(directive)),
    }
}

pub fn list_elements(
    ctx: &ThreadContext,
    list: Word,
    directive: &Directive,
) -> Result<Vec<Word>, FormatError> {
    let mut values = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = list;
    while cursor != Word::NIL {
        if !seen.insert(cursor) || !matches!(classify_object(ctx, cursor), ObjectRef::Cons(_)) {
            return Err(FormatError::at(directive, FormatErrorKind::ImproperList));
        }
        values.push(car(ctx, cursor).map_err(|error| object_error(directive, error))?);
        cursor = cdr(ctx, cursor).map_err(|error| object_error(directive, error))?;
    }
    Ok(values)
}

pub fn string_value(ctx: &ThreadContext, value: StringObject) -> Result<String, ObjectError> {
    (0..string_length(ctx, value.as_word())?)
        .map(|index| string_ref(ctx, value.as_word(), index))
        .collect()
}

pub fn with_rooted_words<T>(
    ctx: &mut ThreadContext,
    words: &[Word],
    directive: &Directive,
    body: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, FormatError>,
) -> Result<T, FormatError> {
    ncl_object::with_rooted_slice(ctx, words, |ctx, rooted| Ok(body(ctx, rooted)))
        .map_err(|error| object_error(directive, error))?
}

pub fn execute(
    kind: ControlDirective,
    directive: &Directive,
    state: &mut State<'_>,
    output: &mut Output<'_>,
) -> FlowResult {
    match kind {
        ControlDirective::Star => arguments::execute_star(directive, state),
        ControlDirective::Conditional => selection::execute(directive, state, output),
        ControlDirective::Iteration => iteration::execute(directive, state, output),
        ControlDirective::Recursive => recursive::execute(directive, state, output),
        ControlDirective::CaseConversion => case::execute(directive, state, output),
        ControlDirective::UpArrow => arguments::execute_up_arrow(directive, state),
    }
}

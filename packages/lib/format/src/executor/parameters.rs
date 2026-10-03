use ncl_object::{ObjectRef, ThreadContext, Word, classify_object, string_length, string_ref};

use super::{ExecutionState, FormatError};
use crate::{Directive, DirectiveKind, Parameter};

pub(super) const fn parameter_i64(parameter: Option<&Parameter>) -> Option<i64> {
    if let Some(Parameter::Integer(value)) = parameter {
        Some(*value)
    } else {
        None
    }
}

pub(super) fn parameter_width(
    parameter: Option<&Parameter>,
    directive: DirectiveKind,
) -> Result<usize, FormatError> {
    match parameter {
        None | Some(Parameter::Unsupplied) => Ok(0),
        Some(Parameter::Integer(value)) if *value >= 0 => {
            Ok(*value as usize)
        }
        Some(
            Parameter::Integer(_)
            | Parameter::Character(_)
            | Parameter::Relative
            | Parameter::ArgumentCount,
        ) => Err(FormatError::InvalidParameter { directive }),
    }
}

pub(super) fn parameter_usize(
    parameter: Option<&Parameter>,
    directive: DirectiveKind,
) -> Result<Option<usize>, FormatError> {
    match parameter {
        None | Some(Parameter::Unsupplied) => Ok(None),
        Some(Parameter::Integer(value)) if *value >= 0 => Ok(Some(*value as usize)),
        Some(
            Parameter::Integer(_)
            | Parameter::Character(_)
            | Parameter::Relative
            | Parameter::ArgumentCount,
        ) => Err(FormatError::InvalidParameter { directive }),
    }
}

pub(super) fn object_string(ctx: &ThreadContext, value: Word) -> Option<String> {
    if !matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        return None;
    }
    let length = string_length(ctx, value).ok()?;
    (0..length)
        .map(|index| string_ref(ctx, value, index).ok())
        .collect()
}

pub(super) fn repeat_count(directive: &Directive) -> Result<usize, FormatError> {
    match directive.parameters.first() {
        None | Some(Parameter::Unsupplied) => Ok(1),
        Some(Parameter::Integer(value)) if *value >= 0 => {
            Ok(*value as usize)
        }
        Some(
            Parameter::Integer(_)
            | Parameter::Character(_)
            | Parameter::Relative
            | Parameter::ArgumentCount,
        ) => Err(FormatError::InvalidParameter {
            directive: directive.kind,
        }),
    }
}

pub(super) fn repeat_count_for(
    directive: &Directive,
    state: &ExecutionState<'_>,
) -> Result<usize, FormatError> {
    if matches!(directive.parameters.first(), Some(Parameter::ArgumentCount)) {
        return Ok(state.arguments.len().saturating_sub(*state.argument_index));
    }
    repeat_count(directive)
}

pub(super) fn is_integer(ctx: &ThreadContext, value: Word) -> bool {
    matches!(
        classify_object(ctx, value),
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_)
    )
}

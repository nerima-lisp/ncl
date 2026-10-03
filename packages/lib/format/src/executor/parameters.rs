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

pub(super) fn parameter_usize(
    parameter: Option<&Parameter>,
    directive: DirectiveKind,
) -> Result<Option<usize>, FormatError> {
    match parameter {
        None | Some(Parameter::Unsupplied) => Ok(None),
        Some(Parameter::Integer(value)) if *value >= 0 => {
            Ok(Some(usize::try_from(*value).map_err(|_| {
                FormatError::InvalidParameter { directive }
            })?))
        }
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
            usize::try_from(*value).map_err(|_| FormatError::InvalidParameter {
                directive: directive.kind,
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_helpers_distinguish_supported_and_invalid_values() {
        assert_eq!(parameter_i64(Some(&Parameter::Integer(-2))), Some(-2));
        assert_eq!(parameter_i64(Some(&Parameter::Character('x'))), None);
        assert_eq!(parameter_usize(None, DirectiveKind::A).ok(), Some(None));
        assert_eq!(
            parameter_usize(Some(&Parameter::Integer(2)), DirectiveKind::A).ok(),
            Some(Some(2))
        );
        assert!(parameter_usize(Some(&Parameter::Integer(-1)), DirectiveKind::A).is_err());
        assert!(parameter_width(Some(&Parameter::Character('x')), DirectiveKind::A).is_err());
        assert!(
            repeat_count(&Directive {
                parameters: vec![Parameter::Character('x')],
                colon: false,
                at_sign: false,
                kind: DirectiveKind::T
            })
            .is_err()
        );
    }
}

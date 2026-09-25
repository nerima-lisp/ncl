//! Conversion of object-layer typed builtin failures into CL conditions.

use ncl_object::{
    ArithmeticError, CellError, ControlError, FileError, LispError, ObjectError, ObjectType,
    Package, PackageError, ProgramError, Runtime, StreamError, ThreadContext, Word,
    pop_root, push_root,
};

use crate::{ConditionError, ConditionIdentifier, ConditionSlotValue, make_typed_condition};

fn words(values: &[Word]) -> Vec<ConditionSlotValue> {
    values
        .iter()
        .copied()
        .map(ConditionSlotValue::from_word)
        .collect()
}

fn fixnum(value: usize) -> Word {
    Word::fixnum(i64::try_from(value).unwrap_or(i64::MAX))
}

fn type_specifier(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expected: ObjectType,
) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
    let name = expected.name().to_ascii_uppercase();
    Package::from_word(package)
        .intern(ctx, runtime, &name)
        .map(|(symbol, _)| symbol)
}

fn object_error(error: ConditionError) -> ObjectError {
    match error {
        ConditionError::Object(error) => error,
        ConditionError::Unhandled
        | ConditionError::NotACondition
        | ConditionError::RestartNotFound
        | ConditionError::ChainCorrupt => ObjectError::Layout,
    }
}

fn type_error_condition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    datum: Word,
    expected: ObjectType,
) -> Result<Word, ObjectError> {
    let mut datum = datum;
    let mut expected_type = type_specifier(ctx, runtime, expected)?;
    let datum_token = push_root(ctx, &mut datum);
    let expected_token = push_root(ctx, &mut expected_type);
    let result = make_typed_condition(
        ctx,
        runtime,
        ConditionIdentifier::TypeError,
        &words(&[datum, expected_type]),
    )
    .map(|record| record.as_word())
    .map_err(object_error);
    pop_root(ctx, expected_token);
    pop_root(ctx, datum_token);
    result
}

/// Construct the standard condition corresponding to a typed builtin error.
pub fn condition_from_lisp_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    error: LispError,
) -> Result<Word, ObjectError> {
    if let LispError::TypeError { datum, expected } = error {
        return type_error_condition(ctx, runtime, datum, expected);
    }
    let (identifier, slots) = match error {
        LispError::TypeError { datum, expected } => (
            ConditionIdentifier::TypeError,
            words(&[datum, type_specifier(ctx, runtime, expected)?]),
        ),
        LispError::ProgramError(error) => match error {
            ProgramError::WrongNumberOfArguments { minimum, maximum } => (
                ConditionIdentifier::ProgramError,
                words(&[fixnum(minimum), maximum.map_or(Word::NIL, fixnum)]),
            ),
            ProgramError::UnknownKeyword | ProgramError::OddKeywordArguments => {
                (ConditionIdentifier::ProgramError, Vec::new())
            }
            _ => (ConditionIdentifier::ProgramError, Vec::new()),
        },
        LispError::ArithmeticError(error) => match error {
            ArithmeticError::DivisionByZero => (ConditionIdentifier::DivisionByZero, Vec::new()),
            ArithmeticError::InvalidOperation => {
                (ConditionIdentifier::ArithmeticError, Vec::new())
            }
            _ => (ConditionIdentifier::ArithmeticError, Vec::new()),
        },
        LispError::ControlError(ControlError::Throw)
        | LispError::ControlError(ControlError::Go)
        | LispError::ControlError(ControlError::ReturnFrom) => {
            (ConditionIdentifier::ControlError, Vec::new())
        }
        LispError::ControlError(_) => (ConditionIdentifier::ControlError, Vec::new()),
        LispError::CellError(error) => (
            match error {
                CellError::UnboundVariable => ConditionIdentifier::UnboundVariable,
                CellError::UndefinedFunction => ConditionIdentifier::UndefinedFunction,
                CellError::UnboundSlot => ConditionIdentifier::UnboundSlot,
                _ => ConditionIdentifier::CellError,
            },
            Vec::new(),
        ),
        LispError::PackageError(error) => (
            ConditionIdentifier::PackageError,
            match error {
                PackageError::NotFound | PackageError::Conflict | PackageError::Locked => Vec::new(),
                _ => Vec::new(),
            },
        ),
        LispError::StreamError(error) => (
            ConditionIdentifier::StreamError,
            match error {
                StreamError::Closed | StreamError::InvalidDirection | StreamError::Io => Vec::new(),
                _ => Vec::new(),
            },
        ),
        LispError::EndOfFile => (ConditionIdentifier::EndOfFile, Vec::new()),
        LispError::FileError(error) => (
            ConditionIdentifier::FileError,
            match error {
                FileError::NotFound | FileError::PermissionDenied | FileError::InvalidPath => {
                    Vec::new()
                }
                _ => Vec::new(),
            },
        ),
        LispError::Object(ObjectError::TypeError) => (
            ConditionIdentifier::TypeError,
            words(&[Word::NIL, Word::NIL]),
        ),
        LispError::Object(ObjectError::Storage(_)) => {
            (ConditionIdentifier::StorageCondition, Vec::new())
        }
        LispError::Object(ObjectError::Layout)
        | LispError::Object(ObjectError::Unbound)
        | LispError::Object(ObjectError::Unsupported)
        | LispError::Object(ObjectError::PackageConflict) => {
            (ConditionIdentifier::Error, Vec::new())
        }
        _ => (ConditionIdentifier::Error, Vec::new()),
    };
    make_typed_condition(ctx, runtime, identifier, &slots)
        .map(|record| record.as_word())
        .map_err(object_error)
}

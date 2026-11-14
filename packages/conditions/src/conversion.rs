//! Conversion of object-layer typed builtin failures into CL conditions.

use ncl_object::typed::{ConditionErrorData, FormatErrorCause, FormatPrintError};
use ncl_object::{
    ArithmeticError, CellError, LispError, ObjectError, ObjectType, Package, Runtime,
    ThreadContext, Word, pop_root, push_root,
};

use crate::{
    ConditionError, ConditionIdentifier, ConditionRecord, ConditionSlotValue, make_typed_condition,
};

fn words(values: &[Word]) -> Vec<ConditionSlotValue> {
    values
        .iter()
        .copied()
        .map(ConditionSlotValue::from_word)
        .collect()
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

const fn object_error(error: ConditionError) -> ObjectError {
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
    let datum_token = push_root(ctx, &mut datum);
    let result = match type_specifier(ctx, runtime, expected) {
        Ok(mut expected_type) => {
            let expected_token = push_root(ctx, &mut expected_type);
            let result = condition_with_word_slots(
                ctx,
                runtime,
                ConditionIdentifier::TypeError,
                &[datum, expected_type],
            );
            pop_root(ctx, expected_token);
            result
        }
        Err(error) => Err(error),
    };
    pop_root(ctx, datum_token);
    result
}

fn condition_with_word_slots(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    identifier: ConditionIdentifier,
    slots: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_rooted_slice(ctx, slots, |ctx, slots| {
        make_typed_condition(ctx, runtime, identifier, &words(slots))
            .map(ConditionRecord::as_word)
            .map_err(object_error)
    })
}

fn condition_from_data(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    error: ConditionErrorData,
) -> Result<Word, ObjectError> {
    match error {
        ConditionErrorData::TypeError {
            datum,
            expected_type,
        } => condition_with_word_slots(
            ctx,
            runtime,
            ConditionIdentifier::TypeError,
            &[datum, expected_type],
        ),
        ConditionErrorData::ArithmeticError {
            kind,
            operation,
            operands,
        } => {
            let identifier = match kind {
                ArithmeticError::DivisionByZero => ConditionIdentifier::DivisionByZero,
                ArithmeticError::InvalidOperation => ConditionIdentifier::ArithmeticError,
            };
            condition_with_word_slots(ctx, runtime, identifier, &[operation, operands])
        }
        ConditionErrorData::PackageError { package, .. } => {
            condition_with_word_slots(ctx, runtime, ConditionIdentifier::PackageError, &[package])
        }
        ConditionErrorData::StreamError { stream, .. } => {
            condition_with_word_slots(ctx, runtime, ConditionIdentifier::StreamError, &[stream])
        }
        ConditionErrorData::EndOfFile { stream } => {
            condition_with_word_slots(ctx, runtime, ConditionIdentifier::EndOfFile, &[stream])
        }
        ConditionErrorData::ReaderError { stream } => {
            condition_with_word_slots(ctx, runtime, ConditionIdentifier::ReaderError, &[stream])
        }
        ConditionErrorData::FileError { pathname, .. } => {
            condition_with_word_slots(ctx, runtime, ConditionIdentifier::FileError, &[pathname])
        }
    }
}

fn simple_error_condition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    format_control: Word,
    format_arguments: Word,
) -> Result<Word, ObjectError> {
    let mut format_control = format_control;
    let mut format_arguments = format_arguments;
    let format_control_token = push_root(ctx, &mut format_control);
    let format_arguments_token = push_root(ctx, &mut format_arguments);
    let result = make_typed_condition(
        ctx,
        runtime,
        ConditionIdentifier::SimpleError,
        &words(&[format_control, format_arguments]),
    )
    .map(ConditionRecord::as_word)
    .map_err(object_error);
    pop_root(ctx, format_arguments_token);
    pop_root(ctx, format_control_token);
    result
}

/// Construct the standard condition corresponding to a typed builtin error.
///
/// # Errors
///
/// Returns an [`ObjectError`] if constructing the condition or resolving its
/// type information fails.
pub fn condition_from_lisp_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    error: LispError,
) -> Result<Word, ObjectError> {
    if let LispError::TypeError { datum, expected } = error {
        return type_error_condition(ctx, runtime, datum, expected);
    }
    if let LispError::CellError(
        CellError::UndefinedFunction { name } | CellError::UnboundVariable { name },
    ) = error
    {
        let identifier = if matches!(
            error,
            LispError::CellError(CellError::UnboundVariable { .. })
        ) {
            ConditionIdentifier::UnboundVariable
        } else {
            ConditionIdentifier::UndefinedFunction
        };
        let mut name = name;
        let token = push_root(ctx, &mut name);
        let result = make_typed_condition(ctx, runtime, identifier, &words(&[name]))
            .map(ConditionRecord::as_word)
            .map_err(object_error);
        pop_root(ctx, token);
        return result;
    }
    let (identifier, slots) = match error {
        LispError::TypeError { datum, expected } => (
            ConditionIdentifier::TypeError,
            words(&[datum, type_specifier(ctx, runtime, expected)?]),
        ),
        LispError::ProgramError(_) => (ConditionIdentifier::ProgramError, Vec::new()),
        LispError::ArithmeticError(error) => match error {
            ArithmeticError::DivisionByZero => (ConditionIdentifier::DivisionByZero, Vec::new()),
            ArithmeticError::InvalidOperation => (ConditionIdentifier::ArithmeticError, Vec::new()),
        },
        LispError::ControlError(_) => (ConditionIdentifier::ControlError, Vec::new()),
        LispError::CellError(error) => (
            match error {
                CellError::UnboundVariable { .. } => ConditionIdentifier::UnboundVariable,
                CellError::UndefinedFunction { .. } => ConditionIdentifier::UndefinedFunction,
                CellError::UnboundSlot { .. } => ConditionIdentifier::UnboundSlot,
            },
            match error {
                CellError::UndefinedFunction { name } | CellError::UnboundVariable { name } => {
                    words(&[name])
                }
                CellError::UnboundSlot { name, instance } => words(&[name, instance]),
            },
        ),
        LispError::PackageError(_) => (ConditionIdentifier::PackageError, Vec::new()),
        LispError::StreamError(_) => (ConditionIdentifier::StreamError, Vec::new()),
        LispError::EndOfFile => (ConditionIdentifier::EndOfFile, Vec::new()),
        LispError::FileError(_) => (ConditionIdentifier::FileError, Vec::new()),
        LispError::SimpleError {
            format_control,
            format_arguments,
        } => return simple_error_condition(ctx, runtime, format_control, format_arguments),
        LispError::FormatError(error) => {
            if let FormatErrorCause::Print(FormatPrintError::Object(error)) = error.cause {
                return condition_from_lisp_error(ctx, runtime, LispError::Object(error));
            }
            return simple_error_condition(
                ctx,
                runtime,
                error.format_control,
                error.format_arguments,
            );
        }
        LispError::Object(ObjectError::TypeError) => (
            ConditionIdentifier::TypeError,
            words(&[Word::NIL, Word::NIL]),
        ),
        LispError::Object(ObjectError::Storage(_)) => {
            (ConditionIdentifier::StorageCondition, Vec::new())
        }
        LispError::Condition(error) => return condition_from_data(ctx, runtime, error),
        LispError::Object(
            ObjectError::Layout
            | ObjectError::Unbound
            | ObjectError::UndefinedFunction
            | ObjectError::NonLocalExit
            | ObjectError::RootStackCorrupted
            // check-added-lines: allow(unsupported) existing error category is mapped, not stubbed
            | ObjectError::Unsupported
            | ObjectError::PackageConflict
            | ObjectError::ControlError,
        ) => (ConditionIdentifier::Error, Vec::new()),
    };
    make_typed_condition(ctx, runtime, identifier, &slots)
        .map(ConditionRecord::as_word)
        .map_err(object_error)
}

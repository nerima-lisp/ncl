//! Typed error condition classes and standard-slot mappings.

use std::error::Error;

use ncl_conditions::{ConditionIdentifier, condition_class_of, condition_from_lisp_error};
use ncl_object::typed::ConditionErrorData;
use ncl_object::{
    ArithmeticError, CellError, ControlError, FileError, HeapRootedWord, HeapRootedWords, Instance,
    LispError, ObjectError, ObjectType, Package, PackageError, ProgramError, Runtime, StreamError,
    ThreadContext, TypeError, Word, make_cons, make_string, slot_ref,
};
use ncl_sys::StorageCondition;

fn setup() -> Result<(Runtime, ThreadContext), ncl_object::ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_conditions::register(&runtime)?;
    Ok((runtime, ctx))
}

fn intern(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package_name: &str,
    symbol_name: &str,
) -> Result<HeapRootedWord, ncl_object::ObjectError> {
    let package = runtime.ensure_package(ctx, package_name)?;
    let (symbol, _) = Package::from_word(package).intern(ctx, runtime, symbol_name)?;
    HeapRootedWord::new(ctx, symbol)
}

fn lisp_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: &str,
) -> Result<HeapRootedWord, ncl_object::ObjectError> {
    let value = value.chars().collect::<Vec<_>>();
    let string = make_string(ctx, runtime, &value)?;
    HeapRootedWord::new(ctx, string)
}

fn lisp_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<HeapRootedWord, ncl_object::ObjectError> {
    let values = HeapRootedWords::new(ctx, values)?;
    let list = HeapRootedWord::new(ctx, Word::NIL)?;
    for value in values.as_slice().iter().rev() {
        list.set(make_cons(ctx, runtime, value.get(), list.get())?);
    }
    Ok(list)
}

fn assert_mapping(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    condition: Word,
    expected_class: ConditionIdentifier,
    expected_slots: &[Word],
) -> Result<(), Box<dyn Error>> {
    let condition = HeapRootedWord::new(ctx, condition)?;
    let expected_slots = HeapRootedWords::new(ctx, expected_slots)?;
    let actual_class = condition_class_of(ctx, condition.get())?;
    let actual_class = HeapRootedWord::new(ctx, actual_class.as_word())?;
    let expected_class = expected_class
        .class(ctx, runtime)
        .ok_or(ncl_object::ObjectError::Layout)?;
    let expected_class = HeapRootedWord::new(ctx, expected_class.as_word())?;
    assert_eq!(actual_class.get(), expected_class.get());

    for (index, expected) in expected_slots.as_slice().iter().enumerate() {
        assert_eq!(
            slot_ref(ctx, Instance::from_word(condition.get()), index)?,
            expected.get(),
            "condition slot {index}"
        );
    }
    Ok(())
}

#[test]
fn type_error_maps_class_and_both_standard_slots() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER")?;
    let expected_type = lisp_list(
        &mut ctx,
        &runtime,
        &[integer.get(), Word::fixnum(1), Word::fixnum(10)],
    )?;
    let datum = Word::fixnum(11);

    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Condition(ConditionErrorData::TypeError {
            datum,
            expected_type: expected_type.get(),
        }),
    )?;

    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::TypeError,
        &[datum, expected_type.get()],
    )
}

#[test]
fn arithmetic_error_maps_operation_and_operands() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let division = intern(&mut ctx, &runtime, "COMMON-LISP", "/")?;
    let division_operands = lisp_list(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(0)])?;

    let division_condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Condition(ConditionErrorData::ArithmeticError {
            kind: ArithmeticError::DivisionByZero,
            operation: division.get(),
            operands: division_operands.get(),
        }),
    )?;

    assert_mapping(
        &mut ctx,
        &runtime,
        division_condition,
        ConditionIdentifier::DivisionByZero,
        &[division.get(), division_operands.get()],
    )?;

    let addition = intern(&mut ctx, &runtime, "COMMON-LISP", "+")?;
    let maximum_fixnum = i64::MAX >> ncl_sys::FIXNUM_TAG_BITS;
    let overflow_operands = lisp_list(
        &mut ctx,
        &runtime,
        &[Word::fixnum(maximum_fixnum), Word::fixnum(1)],
    )?;
    let overflow_condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Condition(ConditionErrorData::ArithmeticError {
            kind: ArithmeticError::InvalidOperation,
            operation: addition.get(),
            operands: overflow_operands.get(),
        }),
    )?;

    assert_mapping(
        &mut ctx,
        &runtime,
        overflow_condition,
        ConditionIdentifier::ArithmeticError,
        &[addition.get(), overflow_operands.get()],
    )
}

#[test]
fn legacy_lisp_errors_map_to_their_condition_classes() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let cases = [
        (
            LispError::ProgramError(ProgramError::UnknownKeyword),
            ConditionIdentifier::ProgramError,
        ),
        (
            LispError::ArithmeticError(ArithmeticError::DivisionByZero),
            ConditionIdentifier::DivisionByZero,
        ),
        (
            LispError::ArithmeticError(ArithmeticError::InvalidOperation),
            ConditionIdentifier::ArithmeticError,
        ),
        (
            LispError::ControlError(ControlError::Throw),
            ConditionIdentifier::ControlError,
        ),
        (
            LispError::PackageError(PackageError::Locked),
            ConditionIdentifier::PackageError,
        ),
        (
            LispError::StreamError(StreamError::Closed),
            ConditionIdentifier::StreamError,
        ),
        (LispError::EndOfFile, ConditionIdentifier::EndOfFile),
        (
            LispError::FileError(FileError::NotFound),
            ConditionIdentifier::FileError,
        ),
        (
            LispError::Object(ObjectError::Storage(StorageCondition::ThreadNotRegistered)),
            ConditionIdentifier::StorageCondition,
        ),
        (
            LispError::Object(ObjectError::Unsupported),
            ConditionIdentifier::Error,
        ),
    ];

    for (error, expected_class) in cases {
        let condition = condition_from_lisp_error(&mut ctx, &runtime, error)?;
        assert_mapping(&mut ctx, &runtime, condition, expected_class, &[])?;
    }

    let datum = Word::fixnum(65);
    let expected_type = intern(&mut ctx, &runtime, "COMMON-LISP", "CHARACTER")?;
    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::from(TypeError {
            datum,
            expected: ObjectType::Character,
        }),
    )?;
    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::TypeError,
        &[datum, expected_type.get()],
    )?;

    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Object(ObjectError::TypeError),
    )?;
    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::TypeError,
        &[Word::NIL, Word::NIL],
    )
}

#[test]
fn package_error_maps_the_affected_package() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let package = runtime.ensure_package(&mut ctx, "NCL-TEST")?;
    let package = HeapRootedWord::new(&ctx, package)?;

    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Condition(ConditionErrorData::PackageError {
            kind: PackageError::Conflict,
            package: package.get(),
        }),
    )?;

    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::PackageError,
        &[package.get()],
    )
}

#[test]
fn stream_error_maps_the_affected_stream_value() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let stream = lisp_string(&mut ctx, &runtime, "closed-stream")?;

    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Condition(ConditionErrorData::StreamError {
            kind: StreamError::Closed,
            stream: stream.get(),
        }),
    )?;

    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::StreamError,
        &[stream.get()],
    )
}

#[test]
fn end_of_file_maps_the_affected_stream() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let stream = lisp_string(&mut ctx, &runtime, "input-stream")?;

    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Condition(ConditionErrorData::EndOfFile {
            stream: stream.get(),
        }),
    )?;

    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::EndOfFile,
        &[stream.get()],
    )
}

#[test]
fn reader_error_maps_the_affected_stream() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let stream = lisp_string(&mut ctx, &runtime, "reader-input")?;

    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Condition(ConditionErrorData::ReaderError {
            stream: stream.get(),
        }),
    )?;

    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::ReaderError,
        &[stream.get()],
    )
}

#[test]
fn file_error_maps_the_affected_pathname_designator() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let pathname = lisp_string(&mut ctx, &runtime, "missing-file.lisp")?;

    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::Condition(ConditionErrorData::FileError {
            kind: FileError::NotFound,
            pathname: pathname.get(),
        }),
    )?;

    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::FileError,
        &[pathname.get()],
    )
}

#[test]
fn cell_errors_map_the_class_and_standard_slots() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let variable_name = intern(&mut ctx, &runtime, "NCL-TEST", "MISSING-VARIABLE")?;
    let variable_condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::CellError(CellError::UnboundVariable {
            name: variable_name.get(),
        }),
    )?;
    assert_mapping(
        &mut ctx,
        &runtime,
        variable_condition,
        ConditionIdentifier::UnboundVariable,
        &[variable_name.get()],
    )?;

    let function_name = intern(&mut ctx, &runtime, "NCL-TEST", "MISSING-FUNCTION")?;
    let function_condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::CellError(CellError::UndefinedFunction {
            name: function_name.get(),
        }),
    )?;
    assert_mapping(
        &mut ctx,
        &runtime,
        function_condition,
        ConditionIdentifier::UndefinedFunction,
        &[function_name.get()],
    )?;

    let slot_name = intern(&mut ctx, &runtime, "NCL-TEST", "UNBOUND-SLOT")?;
    let instance = Word::fixnum(17);
    let slot_condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::CellError(CellError::UnboundSlot {
            name: slot_name.get(),
            instance,
        }),
    )?;
    assert_mapping(
        &mut ctx,
        &runtime,
        slot_condition,
        ConditionIdentifier::UnboundSlot,
        &[slot_name.get(), instance],
    )
}

#[test]
fn wrong_arity_maps_to_slotless_program_error() -> Result<(), Box<dyn Error>> {
    let (runtime, mut ctx) = setup()?;
    let function = intern(&mut ctx, &runtime, "COMMON-LISP", "CAR")?;
    let error = LispError::ProgramError(ProgramError::ArityViolation {
        function: function.get(),
        actual: 0,
        minimum: 2,
        maximum: Some(3),
    });

    let condition = condition_from_lisp_error(&mut ctx, &runtime, error)?;
    assert_mapping(
        &mut ctx,
        &runtime,
        condition,
        ConditionIdentifier::ProgramError,
        &[],
    )?;
    assert!(slot_ref(&ctx, Instance::from_word(condition), 0).is_err());
    assert_eq!(
        error,
        LispError::ProgramError(ProgramError::ArityViolation {
            function: function.get(),
            actual: 0,
            minimum: 2,
            maximum: Some(3),
        })
    );
    Ok(())
}

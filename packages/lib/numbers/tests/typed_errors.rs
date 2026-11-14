//! Mapping tests for numeric errors.

use std::error::Error;

use ncl_conditions::ConditionIdentifier;
use ncl_lib_numbers::NumberError;
use ncl_object::typed::ConditionErrorData;
use ncl_object::{
    ArithmeticError, HeapRootedWord, Instance, LispError, Runtime, ThreadContext, Word, slot_ref,
};

fn assert_arithmetic_mapping(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    error: NumberError,
    kind: ArithmeticError,
    class: ConditionIdentifier,
    display: &str,
) -> Result<(), Box<dyn Error>> {
    assert_eq!(error.to_string(), display);

    let lisp_error = LispError::from(error);
    match &lisp_error {
        LispError::Condition(ConditionErrorData::ArithmeticError {
            kind: actual_kind,
            operation,
            operands,
        }) => {
            assert_eq!(*actual_kind, kind);
            assert_eq!(*operation, Word::fixnum(17));
            assert_eq!(*operands, Word::fixnum(23));
        }
        other => panic!("unexpected number error mapping: {other:?}"),
    }

    let condition = ncl_conditions::condition_from_lisp_error(ctx, runtime, lisp_error)?;
    let condition = HeapRootedWord::new(ctx, condition)?;
    let actual_class = ncl_conditions::condition_class_of(ctx, condition.get())?;
    let actual_class = HeapRootedWord::new(ctx, actual_class.as_word())?;
    let expected_class = class
        .class(ctx, runtime)
        .ok_or(ncl_object::ObjectError::Layout)?;
    let expected_class = HeapRootedWord::new(ctx, expected_class.as_word())?;
    assert_eq!(actual_class.get(), expected_class.get());
    assert_eq!(
        slot_ref(ctx, Instance::from_word(condition.get()), 0)?,
        Word::fixnum(17)
    );
    assert_eq!(
        slot_ref(ctx, Instance::from_word(condition.get()), 1)?,
        Word::fixnum(23)
    );
    Ok(())
}

#[test]
fn number_errors_map_every_variant_to_its_condition_and_slots() -> Result<(), Box<dyn Error>> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ncl_conditions::register(&runtime)?;

    assert_arithmetic_mapping(
        &mut ctx,
        &runtime,
        NumberError::DivisionByZero {
            operation: Word::fixnum(17),
            operands: Word::fixnum(23),
        },
        ArithmeticError::DivisionByZero,
        ConditionIdentifier::DivisionByZero,
        "division by zero",
    )?;
    assert_arithmetic_mapping(
        &mut ctx,
        &runtime,
        NumberError::InvalidOperation {
            operation: Word::fixnum(17),
            operands: Word::fixnum(23),
        },
        ArithmeticError::InvalidOperation,
        ConditionIdentifier::ArithmeticError,
        "invalid arithmetic operation",
    )?;
    Ok(())
}

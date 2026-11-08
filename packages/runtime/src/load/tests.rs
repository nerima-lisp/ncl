use std::rc::Rc;

use ncl_object::{HeapRootedWord, LispError, MultipleValues, ObjectError, ThreadContext, Word};
use ncl_sys::{CodeError, NativeError, NativeOperation, SafepointMapError};

use super::{FileLoadError, finish_file_load};
use crate::{Runtime, RuntimeError};

#[test]
fn code_and_safepoint_errors_become_load_conditions() -> Result<(), String> {
    let runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    let object = Rc::clone(&runtime.object);
    let mut context = ThreadContext::new();
    context
        .register(&object)
        .map_err(|error| format!("register context: {error:?}"))?;
    let pathname = HeapRootedWord::new(&context, Word::NIL)
        .map_err(|error| format!("root pathname: {error:?}"))?;
    let mut values = MultipleValues::new();

    assert_eq!(
        finish_file_load(
            &mut context,
            &object,
            &runtime,
            &pathname,
            false,
            &mut values,
            Err(FileLoadError::Runtime(RuntimeError::Code(
                CodeError::OutOfBounds,
            ))),
        ),
        Err(ObjectError::TypeError)
    );
    assert!(matches!(
        context.take_pending_lisp_error(),
        Some(LispError::SimpleError { .. })
    ));

    assert_eq!(
        finish_file_load(
            &mut context,
            &object,
            &runtime,
            &pathname,
            false,
            &mut values,
            Err(FileLoadError::Runtime(RuntimeError::SafepointMap(
                SafepointMapError::InvalidPc,
            ))),
        ),
        Err(ObjectError::TypeError)
    );
    assert!(matches!(
        context.take_pending_lisp_error(),
        Some(LispError::SimpleError { .. })
    ));
    Ok(())
}

#[test]
fn pending_and_native_failures_are_converted_when_load_finishes() -> Result<(), String> {
    let runtime = Runtime::new().map_err(|error| format!("runtime: {error:?}"))?;
    let object = Rc::clone(&runtime.object);
    let mut context = ThreadContext::new();
    context
        .register(&object)
        .map_err(|error| format!("register context: {error:?}"))?;
    let pathname = HeapRootedWord::new(&context, Word::NIL)
        .map_err(|error| format!("root pathname: {error:?}"))?;
    let mut values = MultipleValues::new();

    crate::native_error::signal_native_load_error(&mut context, &object, "nested failure")
        .map_err(|error| format!("create pending simple error: {error:?}"))?;
    let pending_lisp_error = context
        .take_pending_lisp_error()
        .ok_or_else(|| "pending Lisp error was not set".to_owned())?;
    let converter = object
        .lisp_error_converter()
        .ok_or_else(|| "Lisp error converter missing".to_owned())?;
    let condition = converter(&mut context, &object, pending_lisp_error)
        .map_err(|error| format!("convert pending Lisp error: {error:?}"))?;
    let condition = HeapRootedWord::new(&context, condition)
        .map_err(|error| format!("root condition: {error:?}"))?;
    let condition_word = condition.get();
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        finish_file_load(
            &mut context,
            &object,
            &runtime,
            &pathname,
            false,
            &mut values,
            Err(FileLoadError::Runtime(RuntimeError::PendingCondition {
                condition,
                report: Some("nested failure".to_owned()),
            })),
        ),
        Err(ObjectError::TypeError)
    );
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(context.take_pending_condition(), Some(condition_word));

    let native_failure = crate::native_error::native_failure(NativeError::TypeMismatch {
        operation: NativeOperation::Car,
        operand: 0,
        value: Word::fixnum(7),
    });
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(
        finish_file_load(
            &mut context,
            &object,
            &runtime,
            &pathname,
            false,
            &mut values,
            Err(FileLoadError::Runtime(native_failure)),
        ),
        Err(ObjectError::TypeError)
    );
    let condition = context
        .take_pending_condition()
        .ok_or_else(|| "native failure did not publish a pending condition".to_owned())?;
    let condition_class = ncl_conditions::condition_class_of(&context, condition)
        .map_err(|error| format!("native failure condition is invalid: {error:?}"))?;
    let expected_type_error = ncl_conditions::condition_class(&mut context, &object, "TYPE-ERROR")
        .ok_or_else(|| "TYPE-ERROR class is not registered".to_owned())?;
    // check-added-lines: allow(panic) test-only assertions
    assert_eq!(condition_class, expected_type_error);
    Ok(())
}

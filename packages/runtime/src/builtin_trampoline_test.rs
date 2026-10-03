use super::{
    build_x86_64_stub, dispatch, keyword_check_native, keyword_supplied_native,
    keyword_value_native, make_rest_list_native, record_boundary_error,
    undefined_function_dispatch,
};
use crate::{NativeCondition, Runtime, RuntimeError};
use ncl_object::{ObjectError, ThreadContext, Word};

fn eval(runtime: &mut Runtime, source: &str) -> String {
    let value = runtime
        .eval(source)
        .unwrap_or_else(|error| panic!("evaluating {source:?}: {error:?}"));
    runtime.format_result(value)
}

fn eval_error(runtime: &mut Runtime, source: &str) -> RuntimeError {
    match runtime.eval(source) {
        Ok(value) => panic!(
            "evaluation unexpectedly succeeded with {}",
            runtime.format_result(value)
        ),
        Err(error) => error,
    }
}

#[test]
fn x86_stub_places_pinned_arguments_in_sysv_stack_slots() -> Result<(), String> {
    let bytes = build_x86_64_stub(0x1122_3344_5566_7788)
        .map_err(|error| format!("x86 stub assembly: {error}"))?;
    if !bytes
        .windows(4)
        .any(|window| window == [0x4c, 0x89, 0x14, 0x24])
    {
        return Err("function object is not stored in the seventh argument slot".to_owned());
    }
    if !bytes
        .windows(4)
        .any(|window| window == [0x4c, 0x89, 0x7c, 0x24])
    {
        return Err("thread context is not stored in the eighth argument slot".to_owned());
    }
    Ok(())
}

#[test]
fn boundary_non_local_exit_is_dropped_during_unwind() {
    let mut context = ThreadContext::new();
    context.set_non_local_exit(true);
    record_boundary_error(&mut context, ObjectError::NonLocalExit);
    // check-added-lines: allow(panic) test-only
    assert!(context.take_pending().is_none());
    // check-added-lines: allow(panic) test-only
    assert!(context.is_unwinding());
}

#[test]
fn boundary_non_local_exit_without_unwind_becomes_control_error() {
    let mut context = ThreadContext::new();
    record_boundary_error(&mut context, ObjectError::NonLocalExit);
    // check-added-lines: allow(panic) test-only
    assert_eq!(context.take_pending(), Some(ObjectError::ControlError));
}

#[test]
fn boundary_regular_error_is_recorded() {
    let mut context = ThreadContext::new();
    record_boundary_error(&mut context, ObjectError::TypeError);
    // check-added-lines: allow(panic) test-only
    assert_eq!(context.take_pending(), Some(ObjectError::TypeError));
}

#[test]
fn generic_dispatch_converts_builtin_type_errors_into_conditions() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    assert!(matches!(
        eval_error(&mut runtime, "(car 7)"),
        RuntimeError::NativeFailure {
            condition: NativeCondition::Lisp(ncl_object::LispError::TypeError { .. }),
            ..
        }
    ));
}

#[test]
fn generic_dispatch_reports_wrong_arity_without_calling_the_builtin() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    assert_eq!(
        eval(
            &mut runtime,
            "(handler-case (car) (error (condition) :wrong-arity))",
        ),
        ":WRONG-ARITY"
    );
}

#[test]
fn generic_dispatch_forwards_rest_arguments_beyond_registers() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    assert_eq!(
        eval(
            &mut runtime,
            "(funcall (lambda (&rest values) values) 1 2 3 4 5 6)",
        ),
        "(1 2 3 4 5 6)"
    );
}

#[test]
fn generic_dispatch_adapts_keyword_arguments() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    assert_eq!(
        eval(
            &mut runtime,
            "(funcall (lambda (&key value) value) :value 9)",
        ),
        "9"
    );
}

#[test]
fn undefined_function_dispatch_reports_the_called_symbol() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    assert!(matches!(
        eval_error(&mut runtime, "(missing-trampoline-function)"),
        RuntimeError::UndefinedFunction { name } if name == "MISSING-TRAMPOLINE-FUNCTION"
    ));
}

#[test]
fn native_entrypoints_reject_null_thread_contexts() {
    let null = std::ptr::null_mut();
    assert_eq!(
        make_rest_list_native(null, 0, 0, 0, 0, 0, 0, 0),
        Word::NIL.bits()
    );
    assert_eq!(
        keyword_check_native(null, 0, 0, 0, 0, 0, 0, 0),
        Word::NIL.bits()
    );
    assert_eq!(
        keyword_value_native(null, 0, 0, 0, 0, 0, 0, 0),
        Word::NIL.bits()
    );
    assert_eq!(
        keyword_supplied_native(null, 0, 0, 0, 0, 0, 0, 0),
        Word::NIL.bits()
    );
    assert_eq!(dispatch(0, 0, 0, 0, 0, 0, 0, 0).value, Word::NIL.bits());
    assert_eq!(
        undefined_function_dispatch(0, 0, 0, 0, 0, 0, 0, 0).value,
        Word::NIL.bits()
    );
}

use crate::{Runtime, RuntimeError};
use ncl_object::{LispError, ObjectError, Runtime as ObjectRuntime, ThreadContext, Word};

fn condition_converter_with_pending_object_error(
    context: &mut ThreadContext,
    _runtime: &ObjectRuntime,
    error: LispError,
) -> Result<Word, ObjectError> {
    context.set_pending(ObjectError::ControlError);
    match error {
        LispError::ProgramError(_) => Ok(Word::NIL),
        _ => Err(ObjectError::Layout),
    }
}

#[test]
fn successful_condition_conversion_reports_and_clears_pending_object_error() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    runtime
        .object
        .register_lisp_error_converter(condition_converter_with_pending_object_error);

    assert!(matches!(
        runtime.eval("(car)"),
        Err(RuntimeError::Object(ObjectError::ControlError))
    ));
    assert!(runtime.context.pending_is_none());
    assert!(!runtime.context.thread_access().runtime_error_pending());
    assert!(runtime.context.take_pending_condition().is_none());

    let value = runtime
        .eval("42")
        .unwrap_or_else(|error| panic!("follow-up evaluation: {error:?}"));
    assert_eq!(runtime.format_result(value), "42");
}

#![allow(clippy::unwrap_used)]

use crate::Runtime;
use ncl_object::typed::FunctionDesignator;
use ncl_object::{FunctionArguments, FunctionCaller, MultipleValues, ObjectError, Word};

fn failing_lisp_error_converter(
    context: &mut ncl_object::ThreadContext,
    _runtime: &ncl_object::Runtime,
    _error: ncl_object::LispError,
) -> Result<Word, ObjectError> {
    context.set_pending(ObjectError::ControlError);
    Err(ObjectError::Layout)
}

#[test]
fn builtin_function_caller_drains_errors_from_compiled_entry() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let function = runtime
        .eval("(lambda () 7)")
        .and_then(|word| ncl_object::FunctionObject::try_from(word).map_err(Into::into))
        .unwrap_or_else(|error| panic!("compiled function: {error:?}"));
    runtime
        .object
        .register_lisp_error_converter(failing_lisp_error_converter);
    runtime
        .context
        .set_pending_lisp_error(ncl_object::LispError::EndOfFile);

    let mut caller = ncl_object::BuiltinFunctionCaller;
    let mut values = MultipleValues::new();
    assert_eq!(
        caller.call_function(
            &mut runtime.context,
            &runtime.object,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::Layout)
    );
    assert!(!runtime.context.thread_access().runtime_error_pending());

    let value = runtime
        .eval("42")
        .unwrap_or_else(|error| panic!("follow-up evaluation: {error:?}"));
    assert_eq!(runtime.format_result(value), "42");
}

#[test]
fn native_callback_throw_preserves_fresh_value_through_unwind_protect() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    runtime
        .eval("(defun native-nlx-callback (v) (throw 'x (list v v)))")
        .unwrap_or_else(|error| panic!("native callback definition: {error:?}"));
    runtime
        .eval(
            "(defun native-nlx-gc () \
                (catch 'x \
                  (unwind-protect \
                    (mapcar #'native-nlx-callback (list 7 8)) \
                    (list 99 100))))",
        )
        .unwrap_or_else(|error| panic!("native callback wrapper: {error:?}"));
    runtime.context.set_gc_stress(true);
    runtime.context.set_strict_forwarding(true);
    let value = runtime
        .eval("(native-nlx-gc)")
        .unwrap_or_else(|error| panic!("native callback evaluation: {error:?}"));
    assert_eq!(runtime.format_result(value), "(7 7)");
}

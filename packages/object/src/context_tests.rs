use super::{LispError, ObjectError, PendingRuntimeErrors, ThreadContext};
use crate::Runtime;
use ncl_sys::{NativeError, NativeOperation, Word};
use std::cell::Cell;

fn echo_condition_handler(
    _runtime: std::ptr::NonNull<()>,
    _context: &mut ThreadContext,
    handler: Word,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    if arguments.is_empty() {
        Ok(handler)
    } else {
        Err(ObjectError::Layout)
    }
}

#[test]
fn pending_error_payloads_keep_source_flags_in_sync() {
    let mut context = ThreadContext::new();
    let root = Cell::new(Word::NIL);
    assert!(matches!(
        context.push_heap_root_cell(&root),
        Err(ObjectError::Layout)
    ));
    assert!(context.pending_is_none());
    context.set_pending(ObjectError::ControlError);
    context.set_pending_lisp_error(LispError::EndOfFile);
    assert!(!context.pending_is_none());
    assert!(context.thread.runtime_error_pending());

    assert_eq!(context.take_pending(), Some(ObjectError::ControlError));
    assert!(context.thread.runtime_error_pending());
    assert_eq!(
        context.take_pending_lisp_error(),
        Some(LispError::EndOfFile)
    );
    assert!(!context.thread.runtime_error_pending());
    assert!(context.pending_is_none());

    let handler = Word::fixnum(7);
    assert_eq!(
        context.invoke_condition_handler(handler, &[]),
        Ok(Word::NIL)
    );
    context.set_condition_handler_invoker(echo_condition_handler);
    assert_eq!(
        context.invoke_condition_handler(handler, &[]),
        Err(ObjectError::Layout)
    );
    let mut runtime_marker = ();
    context.set_evaluator_runtime(std::ptr::addr_of_mut!(runtime_marker));
    assert_eq!(context.invoke_condition_handler(handler, &[]), Ok(handler));
    context.clear_evaluator_runtime();
}

#[test]
fn multiple_value_area_rejects_results_over_the_native_limit() {
    let mut context = ThreadContext::default();
    let twenty = (0..ncl_sys::MULTIPLE_VALUE_AREA_WORDS)
        .map(|value| Word::fixnum(i64::try_from(value).unwrap_or(0)))
        .collect::<Vec<_>>();
    assert_eq!(context.set_multiple_value_area(&twenty), Ok(()));
    assert_eq!(context.thread_access().multiple_values(), twenty);

    let twenty_one = (0..=ncl_sys::MULTIPLE_VALUE_AREA_WORDS)
        .map(|value| Word::fixnum(i64::try_from(value).unwrap_or(0)))
        .collect::<Vec<_>>();
    assert_eq!(
        context.set_multiple_value_area(&twenty_one),
        Err(ObjectError::Layout)
    );
}

#[test]
fn taking_pending_runtime_errors_drains_all_three_sources() -> Result<(), ObjectError> {
    let mut context = ThreadContext::new();
    let native = NativeError::NullThread {
        operation: NativeOperation::Car,
    };
    context.set_pending(ObjectError::ControlError);
    context.set_pending_lisp_error(LispError::EndOfFile);
    context.thread.set_native_error(native);

    assert_eq!(
        context.take_pending_runtime_errors(),
        PendingRuntimeErrors {
            object: Some(ObjectError::ControlError),
            lisp: Some(LispError::EndOfFile),
            native: Some(native),
        }
    );
    assert!(!context.thread.runtime_error_pending());

    let runtime = Runtime::new()?;
    let mut context = ThreadContext::new();
    context.register(&runtime)?;
    let common_lisp_user = runtime
        .find_package(&context, "COMMON-LISP-USER")
        .ok_or(ObjectError::Layout)?;
    let common_lisp = runtime
        .find_package(&context, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    let use_list = crate::object_access::get(
        &context,
        common_lisp_user,
        crate::widetag::PACKAGE,
        crate::package::USE_LIST,
    )?;
    crate::object_access::put(
        &mut context,
        common_lisp_user,
        crate::package::USE_LIST,
        Word::fixnum(1),
    )?;
    assert_eq!(
        context.ensure_standard_packages(&runtime),
        Err(ObjectError::Layout)
    );
    crate::object_access::put(
        &mut context,
        common_lisp_user,
        crate::package::USE_LIST,
        use_list,
    )?;
    crate::object_access::put(
        &mut context,
        common_lisp,
        crate::package::USED_BY,
        Word::fixnum(1),
    )?;
    assert_eq!(
        context.ensure_standard_packages(&runtime),
        Err(ObjectError::Layout)
    );
    Ok(())
}

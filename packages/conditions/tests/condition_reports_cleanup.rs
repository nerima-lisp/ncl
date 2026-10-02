#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 15 tests for report formatting, multiple-parent dispatch, and cleanup.

use ncl_conditions::{
    ConditionError, cerror, condition_class, condition_report, find_restart, make_condition,
    pop_cleanup, pop_handler, pop_restart, push_cleanup, push_handler, push_restart,
    signal_matched,
};
use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word, make_cons, make_string};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    (runtime, ctx)
}

fn builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, package, name).unwrap()).unwrap()
}

fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn simple_condition_report_handles_directives_missing_arguments_and_trailing_tilde() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "SIMPLE-CONDITION").unwrap();
    let control = make_string(
        &mut ctx,
        &runtime,
        &"A~a B~A C~q D~".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let first = make_cons(&mut ctx, &runtime, Word::fixnum(7), Word::NIL).unwrap();
    let second = make_cons(&mut ctx, &runtime, Word::fixnum(8), first).unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[control, second]).unwrap();
    assert_eq!(
        condition_report(&ctx, condition),
        Some("A8 B7 C~q D~".to_owned())
    );

    let missing_control = make_string(
        &mut ctx,
        &runtime,
        &"missing ~a ~A".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let missing = make_condition(&mut ctx, &runtime, class, &[missing_control, Word::NIL]).unwrap();
    assert_eq!(
        condition_report(&ctx, missing),
        Some("missing #<OBJECT> #<OBJECT>".to_owned())
    );
}

#[test]
fn multiple_parent_class_dispatch_reaches_the_second_parent() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let custom = symbol(&mut ctx, &runtime, "NCL-W15", "SECOND-PARENT");
    let warning = symbol(&mut ctx, &runtime, "COMMON-LISP", "WARNING");
    let error = symbol(&mut ctx, &runtime, "COMMON-LISP", "ERROR");
    let tail = make_cons(&mut ctx, &runtime, error, Word::NIL).unwrap();
    let parents = make_cons(&mut ctx, &runtime, warning, tail).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, define, &[custom, parents, Word::NIL, Word::NIL]),
        Ok(custom)
    );
    let class = condition_class(&mut ctx, &runtime, "SECOND-PARENT").unwrap();
    let error_class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let handler = push_handler(&mut ctx, &runtime, error_class, callback.as_word()).unwrap();
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(signal_matched(&mut ctx, condition), Ok(true));
    pop_handler(&mut ctx, &runtime, handler);
    assert_eq!(signal_matched(&mut ctx, condition), Ok(false));
}

#[test]
fn cleanup_push_pop_restores_nested_chain_before_unwind() {
    let (runtime, mut ctx) = setup();
    let first = push_cleanup(&mut ctx, &runtime, Word::fixnum(1)).unwrap();
    let second = push_cleanup(&mut ctx, &runtime, Word::fixnum(2)).unwrap();
    pop_cleanup(&mut ctx, second);
    let third = push_cleanup(&mut ctx, &runtime, Word::fixnum(3)).unwrap();
    pop_cleanup(&mut ctx, third);
    pop_cleanup(&mut ctx, first);
    ncl_conditions::unwind(&mut ctx);
    assert!(ctx.take_non_local_exit());
    assert!(!ctx.take_non_local_exit());
}

#[test]
fn cerror_unhandled_path_removes_its_temporary_continue_restart() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    let continue_name =
        make_string(&mut ctx, &runtime, &"CONTINUE".chars().collect::<Vec<_>>()).unwrap();
    assert_eq!(
        cerror(
            &mut ctx,
            &runtime,
            Word::fixnum(19),
            Word::fixnum(20),
            condition,
        ),
        Err(ConditionError::Unhandled)
    );
    assert_eq!(find_restart(&ctx, continue_name), Ok(None));
}

#[test]
fn restart_chain_mixes_handler_and_restart_records_without_leaking_state() {
    let (runtime, mut ctx) = setup();
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let condition = make_condition(&mut ctx, &runtime, warning, &[]).unwrap();
    let handler_fn = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let handler = push_handler(&mut ctx, &runtime, warning, handler_fn.as_word()).unwrap();
    let name = make_string(&mut ctx, &runtime, &"W15-MIXED".chars().collect::<Vec<_>>()).unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::fixnum(55),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(signal_matched(&mut ctx, condition), Ok(true));
    assert_eq!(find_restart(&ctx, name), Ok(Some(restart.as_word())));
    pop_restart(&mut ctx, restart);
    pop_handler(&mut ctx, &runtime, handler);
    assert_eq!(find_restart(&ctx, name), Ok(None));
}

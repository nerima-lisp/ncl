#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 16 coverage for public signal paths, report value kinds, and error
//! conversion categories.

use ncl_conditions::{
    ConditionError, condition_class, condition_from_lisp_error, condition_report, error,
    find_restart, make_condition, pop_handler, pop_restart, push_handler, push_restart, signal,
    signal_matched, warn,
};
use ncl_object::{
    FunctionObject, LispError, ObjectError, Package, Runtime, ThreadContext, Word, make_cons,
    make_string,
};

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
fn public_error_and_warn_paths_distinguish_handled_and_muffled_conditions() {
    let (runtime, mut ctx) = setup();
    let error_class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let warning_class = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let error_condition = make_condition(&mut ctx, &runtime, error_class, &[]).unwrap();
    let warning_condition = make_condition(&mut ctx, &runtime, warning_class, &[]).unwrap();
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let handler = push_handler(&mut ctx, &runtime, error_class, callback.as_word()).unwrap();
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(error(&mut ctx, error_condition), Ok(()));
    assert_eq!(signal_matched(&mut ctx, error_condition), Ok(true));
    pop_handler(&mut ctx, &runtime, handler);
    assert_eq!(
        error(&mut ctx, error_condition),
        Err(ConditionError::Unhandled)
    );
    assert!(ctx.take_non_local_exit());
    assert_eq!(warn(&mut ctx, warning_condition), Ok(()));
    assert_eq!(signal(&mut ctx, warning_condition), Ok(()));
}

#[test]
fn report_describes_string_symbol_and_unknown_values_in_simple_conditions() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "SIMPLE-CONDITION").unwrap();
    let control = make_string(&mut ctx, &runtime, &"~a|~a|~a".chars().collect::<Vec<_>>()).unwrap();
    let string = make_string(&mut ctx, &runtime, &"text".chars().collect::<Vec<_>>()).unwrap();
    let name = symbol(&mut ctx, &runtime, "COMMON-LISP", "W16-SYMBOL");
    let tail = make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).unwrap();
    let middle = make_cons(&mut ctx, &runtime, name, tail).unwrap();
    let arguments = make_cons(&mut ctx, &runtime, string, middle).unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[control, arguments]).unwrap();
    assert_eq!(
        condition_report(&ctx, condition),
        Some("text|W16-SYMBOL|#<OBJECT>".to_owned())
    );
}

#[test]
fn builtin_condition_designators_forward_initargs_into_error_and_warning() {
    let (runtime, mut ctx) = setup();
    let error_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "ERROR");
    let warn_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "WARN");
    let error_name = symbol(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-ERROR");
    let warning_name = symbol(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-WARNING");
    let key = symbol(&mut ctx, &runtime, "KEYWORD", "FORMAT-CONTROL");
    let control = make_string(&mut ctx, &runtime, &"W16 ~a".chars().collect::<Vec<_>>()).unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            error_builtin,
            &[error_name, key, control, Word::fixnum(16)],
        ),
        Err(ObjectError::Unsupported)
    );
    assert!(ctx.take_non_local_exit());
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            warn_builtin,
            &[warning_name, key, control, Word::fixnum(16)],
        ),
        Ok(Word::NIL)
    );
}

#[test]
fn conversion_maps_each_object_error_category_to_a_condition() {
    let (runtime, mut ctx) = setup();
    let cases = [
        (ObjectError::TypeError, "TYPE-ERROR"),
        (ObjectError::Layout, "ERROR"),
        (ObjectError::Unbound, "ERROR"),
        (ObjectError::UndefinedFunction, "ERROR"),
        (ObjectError::NonLocalExit, "ERROR"),
        (ObjectError::RootStackCorrupted, "ERROR"),
        (ObjectError::Unsupported, "ERROR"),
        (ObjectError::PackageConflict, "ERROR"),
        (ObjectError::ControlError, "ERROR"),
    ];
    for (error, expected) in cases {
        let condition =
            condition_from_lisp_error(&mut ctx, &runtime, LispError::Object(error)).unwrap();
        let class = condition_class(&mut ctx, &runtime, expected).unwrap();
        assert_eq!(
            ncl_conditions::condition_class_of(&ctx, condition).unwrap(),
            class
        );
    }
}

#[test]
fn mixed_record_chain_keeps_restart_visible_through_handler_dispatch() {
    let (runtime, mut ctx) = setup();
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let condition = make_condition(&mut ctx, &runtime, warning, &[]).unwrap();
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let handler = push_handler(&mut ctx, &runtime, warning, callback.as_word()).unwrap();
    let name = make_string(
        &mut ctx,
        &runtime,
        &"W16-RESTART".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::fixnum(160),
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

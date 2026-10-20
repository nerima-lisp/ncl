#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Coverage for condition classes, handler dispatch, and restart builtins.

use ncl_conditions::{
    ConditionError, ConditionIdentifier, condition_class, condition_class_name, condition_class_of,
    make_condition, make_typed_condition, pop_handler, pop_restart, push_handler, push_restart,
    signal, signal_matched,
};
use ncl_object::{
    FunctionObject, ObjectError, Runtime, ThreadContext, Word, make_string, string_length,
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

#[allow(clippy::unnecessary_wraps)]
#[allow(clippy::missing_const_for_fn)]
fn failing_handler(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    _arguments: &[Word],
) -> Result<Word, ObjectError> {
    Err(ObjectError::TypeError)
}

#[test]
fn class_helpers_cover_identifier_resolution_and_condition_instances() {
    let (runtime, mut ctx) = setup();
    let type_error = ConditionIdentifier::TypeError
        .class(&mut ctx, &runtime)
        .unwrap();
    let simple_error = ConditionIdentifier::SimpleError
        .class(&mut ctx, &runtime)
        .unwrap();
    let type_name = condition_class_name(&ctx, type_error).unwrap();
    let simple_name = condition_class_name(&ctx, simple_error).unwrap();
    assert_eq!(string_length(&ctx, type_name).unwrap(), "TYPE-ERROR".len());
    assert_eq!(
        string_length(&ctx, simple_name).unwrap(),
        "SIMPLE-ERROR".len()
    );

    let typed =
        make_typed_condition(&mut ctx, &runtime, ConditionIdentifier::TypeError, &[]).unwrap();
    let instance_class = condition_class_of(&ctx, typed.as_word()).unwrap();
    assert_eq!(instance_class, type_error);
    assert_eq!(condition_class(&mut ctx, &runtime, "MISSING-W4"), None);
}

#[test]
fn handler_dispatch_reports_no_match_and_restores_after_failure() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    assert_eq!(signal_matched(&mut ctx, condition), Ok(false));
    assert_eq!(signal(&mut ctx, condition), Err(ConditionError::Unhandled));

    let handler_function = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    let chain = push_handler(&mut ctx, &runtime, class, handler_function.as_word()).unwrap();
    ctx.set_condition_handler_invoker(failing_handler);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(
        signal_matched(&mut ctx, condition),
        Err(ConditionError::Object(ObjectError::TypeError))
    );
    assert_eq!(
        signal_matched(&mut ctx, condition),
        Err(ConditionError::Object(ObjectError::TypeError))
    );
    pop_handler(&mut ctx, &runtime, chain);

    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let warning_condition = make_condition(&mut ctx, &runtime, warning, &[]).unwrap();
    assert_eq!(signal(&mut ctx, warning_condition), Ok(()));
}

#[test]
fn condition_builtins_cover_string_warning_and_unhandled_error_paths() {
    let (runtime, mut ctx) = setup();
    let signal_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "SIGNAL");
    let warn_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "WARN");
    let error_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "ERROR");
    let text = make_string(&mut ctx, &runtime, &"wave4".chars().collect::<Vec<_>>()).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, signal_builtin, &[text, Word::fixnum(4)]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, warn_builtin, &[text]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, error_builtin, &[text]),
        Err(ObjectError::Unsupported)
    );
    assert!(ctx.take_non_local_exit());
    assert_eq!(
        runtime.call_builtin(&mut ctx, warn_builtin, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn restart_builtins_cover_shorthands_accessors_and_designator_edges() {
    let (runtime, mut ctx) = setup();
    for name in [
        "USE-VALUE",
        "STORE-VALUE",
        "CONTINUE",
        "ABORT",
        "MUFFLE-WARNING",
    ] {
        let function = builtin(&runtime, &mut ctx, "COMMON-LISP", name);
        let arguments = if matches!(name, "USE-VALUE" | "STORE-VALUE") {
            vec![Word::fixnum(8)]
        } else {
            Vec::new()
        };
        assert_eq!(
            runtime.call_builtin(&mut ctx, function, &arguments),
            Ok(Word::NIL)
        );
    }

    let type_error = condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let condition = make_condition(
        &mut ctx,
        &runtime,
        type_error,
        &[Word::fixnum(11), Word::fixnum(12)],
    )
    .unwrap();
    for (name, expected) in [
        ("TYPE-ERROR-DATUM", Word::fixnum(11)),
        ("TYPE-ERROR-EXPECTED-TYPE", Word::fixnum(12)),
    ] {
        let function = builtin(&runtime, &mut ctx, "COMMON-LISP", name);
        assert_eq!(
            runtime.call_builtin(&mut ctx, function, &[condition]),
            Ok(expected)
        );
    }
    let accessor = builtin(&runtime, &mut ctx, "COMMON-LISP", "TYPE-ERROR-DATUM");
    assert_eq!(
        runtime.call_builtin(&mut ctx, accessor, &[Word::NIL]),
        Err(ObjectError::TypeError)
    );

    let name = make_string(
        &mut ctx,
        &runtime,
        &"W4-RESTART".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let token = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let find = builtin(&runtime, &mut ctx, "COMMON-LISP", "FIND-RESTART");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[Word::fixnum(3)]),
        Ok(Word::fixnum(3))
    );
    pop_restart(&mut ctx, token);
}

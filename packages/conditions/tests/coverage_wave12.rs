#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 12 coverage for reachable wrapper-error and registration branches.

use ncl_conditions::{
    ConditionError, condition_class, condition_class_of, condition_report, make_condition,
    pop_handler, pop_restart, push_handler, push_restart, signal,
};
use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_cons, make_string,
    string_length, string_ref,
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

fn callback_error(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    _arguments: &[Word],
) -> Result<Word, ObjectError> {
    Err(ObjectError::TypeError)
}

fn text(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).unwrap())
        .map(|index| string_ref(ctx, value, index).unwrap())
        .collect()
}

#[test]
fn condition_builtins_propagate_handler_object_errors() {
    let (runtime, mut ctx) = setup();
    let error_class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, error_class, &[]).unwrap();
    let handler = push_handler(&mut ctx, &runtime, error_class, Word::NIL).unwrap();
    ctx.set_condition_handler_invoker(callback_error);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());

    let signal = builtin(&runtime, &mut ctx, "COMMON-LISP", "SIGNAL");
    let warn = builtin(&runtime, &mut ctx, "COMMON-LISP", "WARN");
    let error = builtin(&runtime, &mut ctx, "COMMON-LISP", "ERROR");
    let cerror = builtin(&runtime, &mut ctx, "COMMON-LISP", "CERROR");
    assert_eq!(
        runtime.call_builtin(&mut ctx, signal, &[condition]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, warn, &[condition]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, error, &[condition]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, cerror, &[Word::NIL, condition]),
        Err(ObjectError::TypeError)
    );
    pop_handler(&mut ctx, &runtime, handler);
}

#[test]
fn warning_without_a_custom_report_uses_the_generic_class_report() {
    let (runtime, mut ctx) = setup();
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let condition = make_condition(&mut ctx, &runtime, warning, &[]).unwrap();
    assert_eq!(
        condition_report(&ctx, condition),
        Some("WARNING condition".to_owned())
    );

    let warn = builtin(&runtime, &mut ctx, "COMMON-LISP", "WARN");
    assert_eq!(
        runtime.call_builtin(&mut ctx, warn, &[condition]),
        Ok(Word::NIL)
    );
    assert_eq!(ncl_conditions::signal(&mut ctx, condition), Ok(()));
}

#[test]
fn make_condition_rejects_unknown_and_non_symbol_class_designators() {
    let (runtime, mut ctx) = setup();
    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let unknown = symbol(&mut ctx, &runtime, "COMMON-LISP", "W12-NO-CLASS");
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[unknown]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[Word::fixnum(12)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn define_condition_rejects_an_unknown_superclass() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let name = symbol(&mut ctx, &runtime, "NCL-W12", "BAD-PARENT");
    let missing = symbol(&mut ctx, &runtime, "COMMON-LISP", "W12-MISSING-PARENT");
    let parents = make_cons(&mut ctx, &runtime, missing, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, define, &[name, parents, Word::NIL, Word::NIL]),
        Err(ObjectError::Layout)
    );
    assert!(condition_class(&mut ctx, &runtime, "BAD-PARENT").is_some());
}

#[test]
fn restart_name_returns_the_common_lisp_interned_symbol() {
    let (runtime, mut ctx) = setup();
    let name = make_string(&mut ctx, &runtime, &"W12-NAMED".chars().collect::<Vec<_>>()).unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let restart_name = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    let returned = runtime
        .call_builtin(&mut ctx, restart_name, &[restart.as_word()])
        .unwrap();
    let expected = symbol(&mut ctx, &runtime, "COMMON-LISP", "W12-NAMED");
    assert_eq!(returned, expected);
    assert_eq!(
        text(&ctx, ncl_object::symbol_name(&ctx, returned).unwrap()),
        "W12-NAMED"
    );
    pop_restart(&mut ctx, restart);
}

#[test]
fn malformed_condition_access_and_signal_boundaries_remain_typed() {
    let (runtime, mut ctx) = setup();
    assert_eq!(
        condition_class_of(&ctx, Word::fixnum(0)),
        Err(ConditionError::NotACondition)
    );
    assert_eq!(
        signal(&mut ctx, Word::fixnum(0)),
        Err(ConditionError::NotACondition)
    );
    let accessor = builtin(&runtime, &mut ctx, "COMMON-LISP", "TYPE-ERROR-DATUM");
    assert_eq!(
        runtime.call_builtin(&mut ctx, accessor, &[Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );
}

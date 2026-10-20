#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Assertions for restart, record-chain, and condition-error edge paths.

use ncl_conditions::{
    ConditionError, compute_restarts, condition_class, find_restart, invoke_restart_by_name,
    make_condition, pop_handler, pop_restart, push_cleanup, push_handler, push_restart,
};
use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, car, cdr, make_string, pop_root,
    push_root,
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
fn condition_error_display_and_object_conversion_are_stable() {
    assert_eq!(ConditionError::Unhandled.to_string(), "unhandled condition");
    assert_eq!(ConditionError::NotACondition.to_string(), "not a condition");
    assert_eq!(
        ConditionError::RestartNotFound.to_string(),
        "restart not found"
    );
    assert_eq!(
        ConditionError::ChainCorrupt.to_string(),
        "corrupt record chain"
    );
    let object_error = ncl_object::ObjectError::TypeError;
    assert_eq!(
        ConditionError::from(object_error).to_string(),
        "object error: TypeError"
    );
}

#[test]
fn missing_named_restart_is_reported_without_mutating_the_chain() {
    let (runtime, mut ctx) = setup();
    let missing = make_string(&mut ctx, &runtime, &"MISSING".chars().collect::<Vec<_>>()).unwrap();
    assert_eq!(
        invoke_restart_by_name(&mut ctx, missing, &[]),
        Err(ncl_conditions::ConditionError::RestartNotFound)
    );
    assert_eq!(compute_restarts(&mut ctx, &runtime).unwrap(), Word::NIL);
}

#[test]
fn restart_builtins_cover_designators_and_named_shorthands() {
    let (runtime, mut ctx) = setup();
    let name = symbol(&mut ctx, &runtime, "COMMON-LISP", "BUILTIN-RESTART");
    let function = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    let continue_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, continue_builtin, &[]),
        Ok(Word::NIL)
    );
    let push = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-RESTART");
    let token = runtime
        .call_builtin(
            &mut ctx,
            push,
            &[name, function.as_word(), Word::NIL, Word::NIL, Word::NIL],
        )
        .unwrap();
    let find = builtin(&runtime, &mut ctx, "COMMON-LISP", "FIND-RESTART");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[Word::fixnum(99)]),
        Ok(Word::fixnum(99))
    );

    let pop = builtin(&runtime, &mut ctx, "NCL-EXT", "POP-RESTART");
    assert_eq!(runtime.call_builtin(&mut ctx, pop, &[token]), Ok(Word::NIL));
}

#[test]
fn restart_and_record_chains_preserve_order_and_depth() {
    let (runtime, mut ctx) = setup();
    let type_error = condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let handler = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let mut first_name =
        make_string(&mut ctx, &runtime, &"FIRST".chars().collect::<Vec<_>>()).unwrap();
    let first_name_root = push_root(&mut ctx, &mut first_name);
    let first = push_restart(
        &mut ctx,
        &runtime,
        first_name,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let mut second_name =
        make_string(&mut ctx, &runtime, &"SECOND".chars().collect::<Vec<_>>()).unwrap();
    let second_name_root = push_root(&mut ctx, &mut second_name);
    let second = push_restart(
        &mut ctx,
        &runtime,
        second_name,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();

    let restarts = compute_restarts(&mut ctx, &runtime).unwrap();
    assert_ne!(restarts, Word::NIL);
    let rest = cdr(&ctx, restarts).unwrap();
    assert_ne!(car(&ctx, rest).unwrap(), Word::NIL);
    assert_eq!(cdr(&ctx, rest).unwrap(), Word::NIL);
    assert!(find_restart(&ctx, second_name).unwrap().is_some());

    pop_restart(&mut ctx, second);
    pop_restart(&mut ctx, first);
    pop_handler(&mut ctx, &runtime, handler);
    pop_root(&mut ctx, second_name_root);
    pop_root(&mut ctx, first_name_root);

    let _cleanup = push_cleanup(&mut ctx, &runtime, Word::NIL).unwrap();
    let _cleanup2 = push_cleanup(&mut ctx, &runtime, Word::NIL).unwrap();
    ncl_conditions::unwind(&mut ctx);
    assert!(ctx.take_non_local_exit());
}

#[test]
fn malformed_restart_records_return_object_errors() {
    let (_runtime, ctx) = setup();
    assert_eq!(
        ncl_conditions::restart_name(&ctx, Word::fixnum(1)),
        Err(ConditionError::Object(ncl_object::ObjectError::TypeError))
    );
    assert_eq!(
        ncl_conditions::find_restart(&ctx, Word::fixnum(1)),
        Ok(None)
    );
}

#[test]
fn condition_accessors_and_conversion_cover_remaining_builtin_edges() {
    let (runtime, mut ctx) = setup();
    let type_error = condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let condition = make_condition(
        &mut ctx,
        &runtime,
        type_error,
        &[Word::fixnum(4), Word::fixnum(5)],
    )
    .unwrap();
    let datum = builtin(&runtime, &mut ctx, "COMMON-LISP", "TYPE-ERROR-DATUM");
    let expected = builtin(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "TYPE-ERROR-EXPECTED-TYPE",
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, datum, &[condition]),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, expected, &[condition]),
        Ok(Word::fixnum(5))
    );
}

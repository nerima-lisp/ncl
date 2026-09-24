#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Behavior tests for the condition system: handler nesting and unwind,
//! restart visibility, non-local exit, the `cerror` continue restart, and
//! type-specific signal dispatch.

use ncl_conditions::{
    ConditionClass, ConditionError, cerror, compute_restarts, condition_class, error, find_restart,
    invoke_restart_by_name, make_condition, pop_handler, pop_restart, push_cleanup, push_handler,
    push_restart, signal, unwind,
};
use ncl_object::{Runtime, ThreadContext, Word, make_string};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    (runtime, ctx)
}

fn class(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> ConditionClass {
    condition_class(ctx, runtime, name).unwrap()
}

#[test]
fn handler_chain_nests_and_unwinds() {
    let (runtime, mut ctx) = setup();
    let type_error = class(&runtime, &mut ctx, "TYPE-ERROR");
    let program_error = class(&runtime, &mut ctx, "PROGRAM-ERROR");

    let first = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let second = push_handler(&mut ctx, &runtime, program_error, Word::NIL).unwrap();

    let condition = make_condition(&mut ctx, &runtime, type_error, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();

    pop_handler(&mut ctx, second);
    pop_handler(&mut ctx, first);

    let condition = make_condition(&mut ctx, &runtime, type_error, &[]).unwrap();
    assert_eq!(signal(&mut ctx, condition), Err(ConditionError::Unhandled));
}

#[test]
fn restart_visibility() {
    let (runtime, mut ctx) = setup();
    let name = make_string(
        &mut ctx,
        &runtime,
        &"MY-RESTART".chars().collect::<Vec<_>>(),
    )
    .unwrap();
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

    assert!(find_restart(&ctx, name).unwrap().is_some());
    assert_ne!(compute_restarts(&mut ctx, &runtime).unwrap(), Word::NIL);

    pop_restart(&mut ctx, restart);
    assert!(find_restart(&ctx, name).unwrap().is_none());
}

#[test]
fn error_raises_non_local_exit_when_unhandled() {
    let (runtime, mut ctx) = setup();
    let program_error = class(&runtime, &mut ctx, "PROGRAM-ERROR");
    let condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();

    assert_eq!(error(&mut ctx, condition), Err(ConditionError::Unhandled));
    assert!(ctx.take_non_local_exit());
}

#[test]
fn continue_restart_can_be_invoked() {
    let (runtime, mut ctx) = setup();
    let name = make_string(&mut ctx, &runtime, &"CONTINUE".chars().collect::<Vec<_>>()).unwrap();
    push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::fixnum(42),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();

    assert_eq!(
        invoke_restart_by_name(&mut ctx, name).unwrap(),
        Word::fixnum(42)
    );
    assert!(ctx.take_non_local_exit());
}

#[test]
fn signal_dispatch_is_type_specific() {
    let (runtime, mut ctx) = setup();
    let type_error = class(&runtime, &mut ctx, "TYPE-ERROR");
    let program_error = class(&runtime, &mut ctx, "PROGRAM-ERROR");
    let error_class = class(&runtime, &mut ctx, "ERROR");

    let chain = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, type_error, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();
    pop_handler(&mut ctx, chain);

    let chain = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();
    assert_eq!(signal(&mut ctx, condition), Err(ConditionError::Unhandled));
    pop_handler(&mut ctx, chain);

    let chain = push_handler(&mut ctx, &runtime, error_class, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();
    pop_handler(&mut ctx, chain);
}

#[test]
fn unhandled_warning_is_muffled() {
    let (runtime, mut ctx) = setup();
    let warning = class(&runtime, &mut ctx, "WARNING");
    let condition = make_condition(&mut ctx, &runtime, warning, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();
}

#[test]
fn cerror_signals_with_a_continue_restart() {
    let (runtime, mut ctx) = setup();
    let error_class = class(&runtime, &mut ctx, "ERROR");
    let chain = push_handler(&mut ctx, &runtime, error_class, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, error_class, &[]).unwrap();

    cerror(&mut ctx, &runtime, Word::NIL, Word::NIL, condition).unwrap();

    pop_handler(&mut ctx, chain);
}

#[test]
fn unwind_marks_the_exit() {
    let (runtime, mut ctx) = setup();
    push_cleanup(&mut ctx, &runtime, Word::NIL).unwrap();
    unwind(&mut ctx);
    assert!(ctx.take_non_local_exit());
}

#![allow(clippy::unnecessary_wraps)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Coverage for restart callable/name-designator and record-chain edges.

use ncl_conditions::{
    ConditionError, compute_restarts, condition_class, find_restart, invoke_restart,
    invoke_restart_by_name, make_condition, pop_handler, pop_restart, push_handler, push_restart,
};
use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_string, push_root,
    simple_vector_ref,
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

#[allow(clippy::unnecessary_wraps)]
fn return_first_argument(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    Ok(arguments.first().copied().unwrap_or(Word::NIL))
}

#[test]
fn callable_restart_invocation_uses_the_condition_function_hook() {
    let (runtime, mut ctx) = setup();
    let function = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    let mut function_word = function.as_word();
    let function_root = push_root(&mut ctx, &mut function_word);
    let mut name = make_string(
        &mut ctx,
        &runtime,
        &"CALLABLE-W3".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let name_root = push_root(&mut ctx, &mut name);
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        function_word,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let current = find_restart(&ctx, name).unwrap().unwrap();
    let stored_function = simple_vector_ref(&ctx, current, 2).unwrap();
    assert!(FunctionObject::try_from(stored_function).is_ok());
    ctx.set_condition_handler_invoker(return_first_argument);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(
        invoke_restart(&mut ctx, current, &[Word::fixnum(31)]),
        Ok(Word::fixnum(31))
    );
    assert!(!ctx.take_non_local_exit());
    pop_restart(&mut ctx, restart);
    ncl_object::pop_root(&mut ctx, name_root);
    ncl_object::pop_root(&mut ctx, function_root);
}

#[test]
fn symbol_designators_find_and_invoke_string_named_restarts() {
    let (runtime, mut ctx) = setup();
    let mut name =
        make_string(&mut ctx, &runtime, &"NAMED-W3".chars().collect::<Vec<_>>()).unwrap();
    let name_root = push_root(&mut ctx, &mut name);
    let marker = Word::fixnum(73);
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        marker,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let designator = symbol(&mut ctx, &runtime, "COMMON-LISP", "NAMED-W3");
    let find = builtin(&runtime, &mut ctx, "COMMON-LISP", "FIND-RESTART");
    let invoke = builtin(&runtime, &mut ctx, "COMMON-LISP", "INVOKE-RESTART");
    let found = runtime.call_builtin(&mut ctx, find, &[designator]).unwrap();
    assert_ne!(found, Word::NIL);
    assert_eq!(
        runtime.call_builtin(&mut ctx, invoke, &[designator]),
        Ok(marker)
    );
    assert!(ctx.take_non_local_exit());
    pop_restart(&mut ctx, restart);
    ncl_object::pop_root(&mut ctx, name_root);
}

#[test]
fn restart_builtins_report_missing_and_malformed_designators() {
    let (runtime, mut ctx) = setup();
    let missing = symbol(&mut ctx, &runtime, "COMMON-LISP", "NO-SUCH-W3");
    let find = builtin(&runtime, &mut ctx, "COMMON-LISP", "FIND-RESTART");
    let invoke = builtin(&runtime, &mut ctx, "COMMON-LISP", "INVOKE-RESTART");
    let restart_name = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[Word::fixnum(9)]),
        Ok(Word::fixnum(9))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, invoke, &[missing]),
        Err(ObjectError::ControlError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, restart_name, &[Word::fixnum(9)]),
        Err(ObjectError::TypeError)
    );
    let missing_name = make_string(
        &mut ctx,
        &runtime,
        &"NO-SUCH-W3".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        invoke_restart_by_name(&mut ctx, missing_name, &[]),
        Err(ConditionError::RestartNotFound)
    );
}

#[test]
fn record_chain_filters_handlers_and_keeps_restart_depths() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let handler = push_handler(&mut ctx, &runtime, class, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    let first_name =
        make_string(&mut ctx, &runtime, &"FIRST-W3".chars().collect::<Vec<_>>()).unwrap();
    let first = push_restart(
        &mut ctx,
        &runtime,
        first_name,
        Word::fixnum(1),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let second_name =
        make_string(&mut ctx, &runtime, &"SECOND-W3".chars().collect::<Vec<_>>()).unwrap();
    let second = push_restart(
        &mut ctx,
        &runtime,
        second_name,
        Word::fixnum(2),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    assert_ne!(compute_restarts(&mut ctx, &runtime).unwrap(), Word::NIL);
    assert_eq!(ncl_conditions::signal(&mut ctx, condition), Ok(()));
    pop_restart(&mut ctx, second);
    pop_restart(&mut ctx, first);
    pop_handler(&mut ctx, &runtime, handler);
}

#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Coverage for remaining registration and dispatch branches.

use ncl_conditions::{
    ConditionError, condition_class, condition_class_of, condition_report, make_condition,
    pop_handler, pop_restart, push_handler, push_restart, signal, signal_matched, warn,
};
use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_cons, make_string,
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

const fn non_local_handler(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    _arguments: &[Word],
) -> Result<Word, ObjectError> {
    Err(ObjectError::NonLocalExit)
}

#[test]
fn handler_dispatch_treats_non_local_exit_as_a_match_and_restores_state() {
    let (runtime, mut ctx) = setup();
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let simple_warning = condition_class(&mut ctx, &runtime, "SIMPLE-WARNING").unwrap();
    let condition = make_condition(&mut ctx, &runtime, simple_warning, &[]).unwrap();
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let chain = push_handler(&mut ctx, &runtime, warning, callback.as_word()).unwrap();
    ctx.set_condition_handler_invoker(non_local_handler);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(signal_matched(&mut ctx, condition), Ok(true));
    pop_handler(&mut ctx, &runtime, chain);
    assert_eq!(signal_matched(&mut ctx, condition), Ok(false));
    assert_eq!(signal(&mut ctx, condition), Ok(()));
    assert_eq!(
        warn(&mut ctx, Word::fixnum(8)),
        Err(ConditionError::NotACondition)
    );
}

#[test]
fn registration_supports_multiple_parents_and_function_reports() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let signal_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "SIGNAL");
    let name = symbol(&mut ctx, &runtime, "NCL-W8", "MULTI-PARENT-W8");
    let warning = symbol(&mut ctx, &runtime, "COMMON-LISP", "WARNING");
    let error = symbol(&mut ctx, &runtime, "COMMON-LISP", "ERROR");
    let parents_tail = make_cons(&mut ctx, &runtime, error, Word::NIL).unwrap();
    let parents = make_cons(&mut ctx, &runtime, warning, parents_tail).unwrap();
    let report_function = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            define,
            &[name, parents, Word::NIL, report_function.as_word()],
        ),
        Ok(name)
    );
    let instance = runtime
        .call_builtin(&mut ctx, signal_builtin, &[name])
        .unwrap();
    assert_eq!(instance, Word::NIL);
    let class = condition_class(&mut ctx, &runtime, "MULTI-PARENT-W8").unwrap();
    let direct = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    assert_eq!(condition_class_of(&ctx, direct).unwrap(), class);
    assert_eq!(
        condition_report(&ctx, direct).as_deref(),
        Some("MULTI-PARENT-W8 condition")
    );
}

#[test]
fn slots_accept_duplicate_keys_and_fill_odd_initarg_values_with_nil() {
    let (runtime, mut ctx) = setup();
    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let accessor = builtin(&runtime, &mut ctx, "COMMON-LISP", "TYPE-ERROR-DATUM");
    let class = symbol(&mut ctx, &runtime, "COMMON-LISP", "TYPE-ERROR");
    let key = symbol(&mut ctx, &runtime, "KEYWORD", "DATUM");
    let first = Word::fixnum(1);
    let second = Word::fixnum(2);
    let duplicate = runtime
        .call_builtin(&mut ctx, make, &[class, key, first, key, second])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, accessor, &[duplicate]),
        Ok(first)
    );
    let odd = runtime.call_builtin(&mut ctx, make, &[class, key]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, accessor, &[odd]),
        Ok(Word::NIL)
    );
}

#[test]
fn restart_builtin_accepts_record_designators_and_cleans_the_record() {
    let (runtime, mut ctx) = setup();
    let invoke = builtin(&runtime, &mut ctx, "COMMON-LISP", "INVOKE-RESTART");
    let find = builtin(&runtime, &mut ctx, "COMMON-LISP", "FIND-RESTART");
    let name = make_string(&mut ctx, &runtime, &"W8-RECORD".chars().collect::<Vec<_>>()).unwrap();
    let record = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::fixnum(64),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, invoke, &[record.as_word(), Word::fixnum(7)]),
        Ok(Word::fixnum(64))
    );
    assert!(ctx.take_non_local_exit());
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[record.as_word()]),
        Ok(record.as_word())
    );
    pop_restart(&mut ctx, record);
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[record.as_word()]),
        Ok(record.as_word())
    );
}

#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Focused coverage for registration, class definition, slots, handlers, and restarts.

use ncl_conditions::{
    condition_class, condition_class_of, condition_report, make_condition, pop_handler,
    pop_restart, push_handler, push_restart, signal, signal_matched,
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

#[test]
fn define_condition_and_slot_builtins_cover_custom_metadata() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let slot_ref_builtin = builtin(&runtime, &mut ctx, "NCL-EXT", "CONDITION-SLOT-REF");
    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let custom = symbol(&mut ctx, &runtime, "NCL-W7", "COVERAGE-W7");
    let keyword = symbol(&mut ctx, &runtime, "KEYWORD", "VALUE");
    let pair = make_cons(&mut ctx, &runtime, keyword, Word::NIL).unwrap();
    let specs = make_cons(&mut ctx, &runtime, pair, Word::NIL).unwrap();
    let report = make_string(
        &mut ctx,
        &runtime,
        &"wave7 report".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, define, &[custom, Word::NIL, specs, report]),
        Ok(custom)
    );
    let instance = runtime
        .call_builtin(&mut ctx, make, &[custom, keyword, Word::fixnum(73)])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_ref_builtin, &[instance, Word::fixnum(0)]),
        Ok(Word::fixnum(73))
    );
    assert_eq!(
        condition_report(&ctx, instance).as_deref(),
        Some("wave7 report")
    );
    let custom_class = condition_class(&mut ctx, &runtime, "COVERAGE-W7").unwrap();
    assert_eq!(condition_class_of(&ctx, instance).unwrap(), custom_class);
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_ref_builtin, &[instance, Word::fixnum(-1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_ref_builtin, &[Word::NIL, Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn condition_builtins_cover_symbol_designators_and_existing_conditions() {
    let (runtime, mut ctx) = setup();
    let signal_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "SIGNAL");
    let warn_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "WARN");
    let cerror_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "CERROR");
    let simple_warning = symbol(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-WARNING");
    let control = symbol(&mut ctx, &runtime, "KEYWORD", "FORMAT-CONTROL");
    let text = make_string(
        &mut ctx,
        &runtime,
        &"warning ~a".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, signal_builtin, &[simple_warning, control, text]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, warn_builtin, &[simple_warning, control, text]),
        Ok(Word::NIL)
    );
    let error_class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, error_class, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, cerror_builtin, &[Word::NIL, condition]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, signal_builtin, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn handlers_match_parent_classes_and_restore_the_cluster() {
    let (runtime, mut ctx) = setup();
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let simple_warning = condition_class(&mut ctx, &runtime, "SIMPLE-WARNING").unwrap();
    let condition = make_condition(&mut ctx, &runtime, simple_warning, &[]).unwrap();
    let handler_function = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let chain = push_handler(&mut ctx, &runtime, warning, handler_function.as_word()).unwrap();
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(signal_matched(&mut ctx, condition), Ok(true));
    assert_eq!(signal(&mut ctx, condition), Ok(()));
    pop_handler(&mut ctx, &runtime, chain);
    assert_eq!(signal_matched(&mut ctx, condition), Ok(false));
}

#[test]
fn restart_builtins_cover_named_invocation_name_and_cleanup() {
    let (runtime, mut ctx) = setup();
    let push = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-RESTART");
    let pop = builtin(&runtime, &mut ctx, "NCL-EXT", "POP-RESTART");
    let invoke = builtin(&runtime, &mut ctx, "COMMON-LISP", "INVOKE-RESTART");
    let restart_name = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    let name = make_string(&mut ctx, &runtime, &"W7-NAMED".chars().collect::<Vec<_>>()).unwrap();
    let designator = symbol(&mut ctx, &runtime, "COMMON-LISP", "W7-NAMED");
    let token = runtime
        .call_builtin(
            &mut ctx,
            push,
            &[name, Word::fixnum(88), Word::NIL, Word::NIL, Word::NIL],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, invoke, &[designator]),
        Ok(Word::fixnum(88))
    );
    assert!(ctx.take_non_local_exit());
    assert_ne!(
        runtime.call_builtin(&mut ctx, restart_name, &[token]),
        Ok(Word::NIL)
    );
    assert_eq!(runtime.call_builtin(&mut ctx, pop, &[token]), Ok(Word::NIL));
    assert_eq!(
        runtime.call_builtin(&mut ctx, invoke, &[designator]),
        Err(ObjectError::ControlError)
    );

    let anonymous = push_restart(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    assert_eq!(
        ncl_conditions::restart_name(&ctx, anonymous.as_word()),
        Ok(Word::NIL)
    );
    pop_restart(&mut ctx, anonymous);
}

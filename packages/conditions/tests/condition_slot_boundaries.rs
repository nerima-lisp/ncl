#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Focused coverage for slots, class boundaries, restart records, and builtins.

use ncl_conditions::{
    ConditionError, ConditionIdentifier, condition_class, condition_class_name, condition_class_of,
    find_restart, invoke_restart, invoke_restart_by_name, make_condition_record, pop_handler,
    pop_restart, push_handler, push_restart, restart_name,
};
use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, car, cdr, make_string,
    string_length,
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
fn make_condition_maps_builtin_slot_initargs_and_defaults() {
    let (runtime, mut ctx) = setup();
    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let control_accessor = builtin(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "SIMPLE-CONDITION-FORMAT-CONTROL",
    );
    let arguments_accessor = builtin(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "SIMPLE-CONDITION-FORMAT-ARGUMENTS",
    );
    let class = symbol(&mut ctx, &runtime, "COMMON-LISP", "SIMPLE-CONDITION");
    let control_key = symbol(&mut ctx, &runtime, "KEYWORD", "FORMAT-CONTROL");
    let argument_key = symbol(&mut ctx, &runtime, "KEYWORD", "FORMAT-ARGUMENTS");
    let control = make_string(&mut ctx, &runtime, &"wave6 ~a".chars().collect::<Vec<_>>()).unwrap();
    let arguments = Word::fixnum(42);
    let condition = runtime
        .call_builtin(
            &mut ctx,
            make,
            &[class, control_key, control, argument_key, arguments],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, control_accessor, &[condition]),
        Ok(control)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, arguments_accessor, &[condition]),
        Ok(arguments)
    );

    let defaults = runtime.call_builtin(&mut ctx, make, &[class]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, control_accessor, &[defaults]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, arguments_accessor, &[defaults]),
        Ok(Word::NIL)
    );
}

#[test]
fn class_and_record_boundaries_preserve_class_identity_and_errors() {
    let (runtime, mut ctx) = setup();
    let class = ConditionIdentifier::ArithmeticError
        .class(&mut ctx, &runtime)
        .unwrap();
    let name = condition_class_name(&ctx, class).unwrap();
    assert_eq!(string_length(&ctx, name).unwrap(), "ARITHMETIC-ERROR".len());
    let record = make_condition_record(&mut ctx, &runtime, class, &[]).unwrap();
    assert_eq!(condition_class_of(&ctx, record.as_word()).unwrap(), class);
    assert_eq!(
        condition_class_of(&ctx, Word::fixnum(0)),
        Err(ConditionError::NotACondition)
    );
    assert_eq!(
        condition_class_name(
            &ctx,
            ncl_conditions::ConditionClass::from_word(Word::fixnum(0))
        ),
        Err(ConditionError::Object(ObjectError::TypeError))
    );
    assert_eq!(condition_class(&mut ctx, &runtime, "NO-WAVE6-CLASS"), None);
}

#[test]
fn restart_records_cover_anonymous_names_markers_and_mixed_chains() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let handler = push_handler(&mut ctx, &runtime, class, Word::NIL).unwrap();
    let name = make_string(
        &mut ctx,
        &runtime,
        &"WAVE6-RESTART".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::fixnum(91),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let restarts = ncl_conditions::compute_restarts(&mut ctx, &runtime).unwrap();
    assert_ne!(restarts, Word::NIL);
    assert_eq!(car(&ctx, restarts).unwrap(), restart.as_word());
    assert_eq!(cdr(&ctx, restarts).unwrap(), Word::NIL);
    assert_eq!(find_restart(&ctx, name).unwrap(), Some(restart.as_word()));
    assert_eq!(
        invoke_restart(&mut ctx, restart.as_word(), &[]),
        Ok(Word::fixnum(91))
    );
    assert!(ctx.take_non_local_exit());
    assert_eq!(restart_name(&ctx, restart.as_word()), Ok(name));
    pop_restart(&mut ctx, restart);
    pop_handler(&mut ctx, &runtime, handler);

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
    assert_eq!(restart_name(&ctx, anonymous.as_word()), Ok(Word::NIL));
    pop_restart(&mut ctx, anonymous);
    let missing_name = make_string(
        &mut ctx,
        &runtime,
        &"MISSING-W6".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        invoke_restart_by_name(&mut ctx, missing_name, &[]),
        Err(ConditionError::RestartNotFound)
    );
}

#[test]
fn restart_builtins_cover_symbol_designators_and_ignored_rest() {
    let (runtime, mut ctx) = setup();
    let push = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-RESTART");
    let pop = builtin(&runtime, &mut ctx, "NCL-EXT", "POP-RESTART");
    let find = builtin(&runtime, &mut ctx, "COMMON-LISP", "FIND-RESTART");
    let compute = builtin(&runtime, &mut ctx, "COMMON-LISP", "COMPUTE-RESTARTS");
    let designator = symbol(&mut ctx, &runtime, "COMMON-LISP", "WAVE6-BUILTIN");
    let name = make_string(
        &mut ctx,
        &runtime,
        &"WAVE6-BUILTIN".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let token = runtime
        .call_builtin(
            &mut ctx,
            push,
            &[name, Word::fixnum(7), Word::NIL, Word::NIL, Word::NIL],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[designator, Word::fixnum(99)]),
        Ok(token)
    );
    assert_ne!(
        runtime.call_builtin(&mut ctx, compute, &[Word::fixnum(1)]),
        Ok(Word::NIL)
    );
    assert_eq!(runtime.call_builtin(&mut ctx, pop, &[token]), Ok(Word::NIL));
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[designator]),
        Ok(Word::NIL)
    );
}

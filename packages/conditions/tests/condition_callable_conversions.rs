#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 17 coverage for callable failure boundaries and named conversion slots.

use ncl_conditions::{
    ConditionError, condition_class, condition_class_of, condition_from_lisp_error, find_restart,
    pop_restart, push_restart,
};
use ncl_object::{
    CellError, FunctionObject, LispError, ObjectError, Package, Runtime, ThreadContext, Word,
    make_cons, make_string, slot_ref, string_length, string_ref,
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

fn text(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).unwrap())
        .map(|index| string_ref(ctx, value, index).unwrap())
        .collect()
}

fn symbol_text(ctx: &ThreadContext, value: Word) -> String {
    text(ctx, ncl_object::symbol_name(ctx, value).unwrap())
}

const fn callback_error(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    _arguments: &[Word],
) -> Result<Word, ObjectError> {
    Err(ObjectError::TypeError)
}

#[test]
fn callable_restart_failure_crosses_condition_error_boundary() {
    let (runtime, mut ctx) = setup();
    let function = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let name = make_string(
        &mut ctx,
        &runtime,
        &"W17-CALLABLE".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        function.as_word(),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    ctx.set_condition_handler_invoker(callback_error);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(
        ncl_conditions::invoke_restart(&mut ctx, restart.as_word(), &[Word::fixnum(17)]),
        Err(ConditionError::Object(ObjectError::TypeError))
    );
    pop_restart(&mut ctx, restart);
}

#[test]
fn slot_initform_failure_is_returned_by_make_condition() {
    let (runtime, mut ctx) = setup();
    let define = builtin(&runtime, &mut ctx, "NCL-EXT", "DEFINE-CONDITION-CLASS");
    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let name = symbol(&mut ctx, &runtime, "NCL-W17", "BAD-INITFORM");
    let initarg = symbol(&mut ctx, &runtime, "KEYWORD", "VALUE");
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let spec = make_cons(&mut ctx, &runtime, initarg, callback.as_word()).unwrap();
    let specs = make_cons(&mut ctx, &runtime, spec, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, define, &[name, Word::NIL, specs, Word::NIL]),
        Ok(name)
    );
    ctx.set_condition_handler_invoker(callback_error);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[name]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn named_cell_error_conversions_keep_the_name_slot_for_both_variants() {
    let (runtime, mut ctx) = setup();
    for (error, expected_class, expected_name) in [
        (
            LispError::CellError(CellError::UnboundVariable {
                name: symbol(&mut ctx, &runtime, "COMMON-LISP", "W17-VARIABLE"),
            }),
            "UNBOUND-VARIABLE",
            "W17-VARIABLE",
        ),
        (
            LispError::CellError(CellError::UndefinedFunction {
                name: symbol(&mut ctx, &runtime, "COMMON-LISP", "W17-FUNCTION"),
            }),
            "UNDEFINED-FUNCTION",
            "W17-FUNCTION",
        ),
    ] {
        let condition = condition_from_lisp_error(&mut ctx, &runtime, error).unwrap();
        let class = condition_class(&mut ctx, &runtime, expected_class).unwrap();
        assert_eq!(condition_class_of(&ctx, condition).unwrap(), class);
        assert_eq!(
            symbol_text(
                &ctx,
                slot_ref(&ctx, ncl_object::Instance::from_word(condition), 0).unwrap()
            ),
            expected_name
        );
    }
}

#[test]
fn malformed_restart_words_fail_without_changing_an_active_restart() {
    let (runtime, mut ctx) = setup();
    let name = make_string(
        &mut ctx,
        &runtime,
        &"W17-ACTIVE".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::fixnum(9),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    assert_eq!(
        ncl_conditions::restart_name(&ctx, Word::fixnum(1)),
        Err(ConditionError::Object(ObjectError::TypeError))
    );
    assert_eq!(
        ncl_conditions::invoke_restart(&mut ctx, Word::fixnum(1), &[]),
        Err(ConditionError::Object(ObjectError::TypeError))
    );
    assert!(find_restart(&ctx, name).unwrap().is_some());
    pop_restart(&mut ctx, restart);
}

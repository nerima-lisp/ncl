#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Focused coverage for handler-driven restart cleanup and cleanup-chain reset.

use ncl_conditions::{
    ConditionError, cerror, condition_class, condition_class_of, condition_from_lisp_error,
    find_restart, make_condition, pop_cleanup, pop_handler, push_cleanup, push_handler,
};
use ncl_object::{
    ArithmeticError, CellError, FunctionObject, LispError, ObjectError, Package, ProgramError,
    Runtime, ThreadContext, Word, make_string, string_length, string_ref,
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

fn class_name(ctx: &ThreadContext, class: ncl_conditions::ConditionClass) -> String {
    let name = ncl_conditions::condition_class_name(ctx, class).unwrap();
    (0..string_length(ctx, name).unwrap())
        .map(|index| string_ref(ctx, name, index).unwrap())
        .collect()
}

#[allow(
    clippy::missing_const_for_fn,
    clippy::unnecessary_wraps,
    reason = "the callback must match the evaluator hook's Result signature"
)]
fn invoke_continue(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    _arguments: &[Word],
) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(29))
}

#[test]
fn cerror_handler_path_removes_temporary_continue_restart() {
    let (runtime, mut ctx) = setup();
    let error_class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, error_class, &[]).unwrap();
    let handler_function = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let handler =
        push_handler(&mut ctx, &runtime, error_class, handler_function.as_word()).unwrap();
    let continue_name =
        make_string(&mut ctx, &runtime, &"CONTINUE".chars().collect::<Vec<_>>()).unwrap();
    ctx.set_condition_handler_invoker(invoke_continue);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());

    assert_eq!(
        cerror(
            &mut ctx,
            &runtime,
            Word::fixnum(11),
            Word::fixnum(12),
            condition
        ),
        Ok(())
    );
    assert_eq!(find_restart(&ctx, continue_name), Ok(None));
    assert!(!ctx.take_non_local_exit());

    pop_handler(&mut ctx, &runtime, handler);
}

#[test]
fn named_callable_restart_passes_arguments_and_returns_callback_value() {
    let (runtime, mut ctx) = setup();
    let name = make_string(
        &mut ctx,
        &runtime,
        &"NAMED-CALLABLE".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let function = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let restart = ncl_conditions::push_restart(
        &mut ctx,
        &runtime,
        name,
        function.as_word(),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let designator = symbol(&mut ctx, &runtime, "COMMON-LISP", "NAMED-CALLABLE");
    ctx.set_condition_handler_invoker(|_, _, _, arguments| {
        assert_eq!(arguments, &[Word::fixnum(41), Word::fixnum(42)]);
        Ok(Word::fixnum(99))
    });
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());

    let invoke = builtin(&runtime, &mut ctx, "COMMON-LISP", "INVOKE-RESTART");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            invoke,
            &[designator, Word::fixnum(41), Word::fixnum(42)],
        ),
        Ok(Word::fixnum(99))
    );
    assert!(!ctx.take_non_local_exit());
    ncl_conditions::pop_restart(&mut ctx, restart);
    assert_eq!(find_restart(&ctx, name), Ok(None));
}

#[test]
fn conversion_preserves_specific_program_arithmetic_and_cell_classes() {
    let (runtime, mut ctx) = setup();
    let cases = [
        (
            LispError::ProgramError(ProgramError::WrongNumberOfArguments {
                minimum: 2,
                maximum: Some(5),
            }),
            "PROGRAM-ERROR",
        ),
        (
            LispError::ArithmeticError(ArithmeticError::InvalidOperation),
            "ARITHMETIC-ERROR",
        ),
        (LispError::CellError(CellError::UnboundSlot), "UNBOUND-SLOT"),
    ];
    for (error, expected_name) in cases {
        let condition = condition_from_lisp_error(&mut ctx, &runtime, error).unwrap();
        let class = condition_class_of(&ctx, condition).unwrap();
        assert_eq!(class_name(&ctx, class), expected_name);
    }
    let name = symbol(&mut ctx, &runtime, "COMMON-LISP", "MISSING-VARIABLE");
    let condition = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::CellError(CellError::UnboundVariable { name }),
    )
    .unwrap();
    assert_eq!(
        condition_class_of(&ctx, condition).unwrap(),
        condition_class(&mut ctx, &runtime, "UNBOUND-VARIABLE").unwrap()
    );
}

#[test]
fn unwind_clears_cleanup_chain_before_a_later_push() {
    let (runtime, mut ctx) = setup();
    let first = push_cleanup(&mut ctx, &runtime, Word::fixnum(1)).unwrap();
    ncl_conditions::unwind(&mut ctx);
    assert!(ctx.take_non_local_exit());

    let second = push_cleanup(&mut ctx, &runtime, Word::fixnum(2)).unwrap();
    pop_cleanup(&mut ctx, second);
    assert!(!ctx.take_non_local_exit());

    ncl_conditions::unwind(&mut ctx);
    assert!(ctx.take_non_local_exit());
    pop_cleanup(&mut ctx, first);
}

#[test]
fn cerror_without_handler_reports_unhandled_and_leaves_no_restart() {
    let (runtime, mut ctx) = setup();
    let error_class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, error_class, &[]).unwrap();
    let continue_name =
        make_string(&mut ctx, &runtime, &"CONTINUE".chars().collect::<Vec<_>>()).unwrap();

    assert_eq!(
        cerror(
            &mut ctx,
            &runtime,
            Word::fixnum(7),
            Word::fixnum(8),
            condition
        ),
        Err(ConditionError::Unhandled)
    );
    assert_eq!(find_restart(&ctx, continue_name), Ok(None));
}

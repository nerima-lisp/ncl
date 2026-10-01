#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Coverage for matched condition builtins, remaining accessors, and conversions.

use ncl_conditions::{
    ConditionError, ConditionIdentifier, condition_class, condition_class_of,
    condition_from_lisp_error, make_condition, pop_handler, push_handler, signal_matched,
};
use ncl_object::{
    ArithmeticError, ControlError, FileError, FunctionObject, LispError, ObjectError, Package,
    Runtime, StreamError, ThreadContext, Word, make_string,
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

fn class_name(ctx: &ThreadContext, condition: Word) -> String {
    let class = condition_class_of(ctx, condition).unwrap();
    let name = ncl_conditions::condition_class_name(ctx, class).unwrap();
    (0..ncl_object::string_length(ctx, name).unwrap())
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
}

fn non_local_handler(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    _arguments: &[Word],
) -> Result<Word, ObjectError> {
    Err(ObjectError::NonLocalExit)
}

#[test]
fn matched_signal_error_warn_and_cerror_builtins_return_nil() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let chain = push_handler(&mut ctx, &runtime, class, callback.as_word()).unwrap();
    ctx.set_condition_handler_invoker(non_local_handler);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    for name in ["SIGNAL", "ERROR", "WARN"] {
        let function = builtin(&runtime, &mut ctx, "COMMON-LISP", name);
        assert_eq!(
            runtime.call_builtin(&mut ctx, function, &[condition]),
            Ok(Word::NIL)
        );
    }
    let cerror = builtin(&runtime, &mut ctx, "COMMON-LISP", "CERROR");
    assert_eq!(
        runtime.call_builtin(&mut ctx, cerror, &[Word::NIL, condition]),
        Ok(Word::NIL)
    );
    pop_handler(&mut ctx, &runtime, chain);
    assert_eq!(signal_matched(&mut ctx, condition), Ok(false));
}

#[test]
fn restart_and_condition_accessors_cover_arithmetic_slots_and_names() {
    let (runtime, mut ctx) = setup();
    let operation = symbol(&mut ctx, &runtime, "COMMON-LISP", "+");
    let operands = Word::fixnum(3);
    let class = condition_class(&mut ctx, &runtime, "ARITHMETIC-ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[operation, operands]).unwrap();
    for (name, expected) in [
        ("ARITHMETIC-ERROR-OPERATION", operation),
        ("ARITHMETIC-ERROR-OPERANDS", operands),
    ] {
        let accessor = builtin(&runtime, &mut ctx, "COMMON-LISP", name);
        assert_eq!(
            runtime.call_builtin(&mut ctx, accessor, &[condition]),
            Ok(expected)
        );
    }

    let name = make_string(&mut ctx, &runtime, &"W9-NAME".chars().collect::<Vec<_>>()).unwrap();
    let restart = ncl_conditions::push_restart(
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
    assert_ne!(
        runtime.call_builtin(&mut ctx, restart_name, &[restart.as_word()]),
        Ok(Word::NIL)
    );
    ncl_conditions::pop_restart(&mut ctx, restart);
}

#[test]
fn conversion_maps_remaining_object_and_typed_error_categories() {
    let (runtime, mut ctx) = setup();
    let cases = [
        (LispError::ControlError(ControlError::Go), "CONTROL-ERROR"),
        (LispError::StreamError(StreamError::Io), "STREAM-ERROR"),
        (LispError::FileError(FileError::InvalidPath), "FILE-ERROR"),
        (LispError::Object(ObjectError::UndefinedFunction), "ERROR"),
        (LispError::Object(ObjectError::NonLocalExit), "ERROR"),
        (LispError::Object(ObjectError::RootStackCorrupted), "ERROR"),
        (LispError::Object(ObjectError::PackageConflict), "ERROR"),
        (LispError::Object(ObjectError::ControlError), "ERROR"),
    ];
    for (error, expected) in cases {
        let condition = condition_from_lisp_error(&mut ctx, &runtime, error).unwrap();
        assert_eq!(class_name(&ctx, condition), expected);
    }
    let division = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::ArithmeticError(ArithmeticError::DivisionByZero),
    )
    .unwrap();
    assert_eq!(
        class_name(&ctx, division),
        ConditionIdentifier::DivisionByZero.name()
    );
}

#[test]
fn malformed_condition_arguments_stay_at_object_error_boundary() {
    let (runtime, mut ctx) = setup();
    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    let signal = builtin(&runtime, &mut ctx, "COMMON-LISP", "SIGNAL");
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, signal, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ncl_conditions::condition_class_of(&ctx, Word::fixnum(1)),
        Err(ConditionError::NotACondition)
    );
}

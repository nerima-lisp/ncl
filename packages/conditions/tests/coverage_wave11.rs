#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 11 coverage for string condition arguments, restart shorthands, and
//! conversion/accessor edge payloads.

use ncl_conditions::{
    ConditionError, condition_class, condition_class_name, condition_class_of,
    condition_from_lisp_error, make_condition, pop_restart, push_restart, signal_matched,
};
use ncl_object::{
    FunctionObject, LispError, ObjectError, Package, PackageError, ProgramError, Runtime,
    StreamError, ThreadContext, Word, make_string, slot_ref, string_length, string_ref,
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

fn string_value(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).unwrap())
        .map(|index| string_ref(ctx, value, index).unwrap())
        .collect()
}

fn class_name(ctx: &ThreadContext, condition: Word) -> String {
    let class = condition_class_of(ctx, condition).unwrap();
    string_value(ctx, condition_class_name(ctx, class).unwrap())
}

fn return_first_argument(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    Ok(arguments.first().copied().unwrap_or(Word::NIL))
}

#[test]
fn string_condition_arguments_build_and_signal_with_format_values() {
    let (runtime, mut ctx) = setup();
    let signal = builtin(&runtime, &mut ctx, "COMMON-LISP", "SIGNAL");
    let warn = builtin(&runtime, &mut ctx, "COMMON-LISP", "WARN");
    let text = make_string(&mut ctx, &runtime, &"wave11 ~a".chars().collect::<Vec<_>>()).unwrap();
    let value = Word::fixnum(11);

    assert_eq!(
        runtime.call_builtin(&mut ctx, signal, &[text, value]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, warn, &[text, value]),
        Ok(Word::NIL)
    );

    let simple = condition_class(&mut ctx, &runtime, "SIMPLE-CONDITION").unwrap();
    let direct = make_condition(&mut ctx, &runtime, simple, &[text, value]).unwrap();
    let control = builtin(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "SIMPLE-CONDITION-FORMAT-CONTROL",
    );
    let arguments = builtin(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "SIMPLE-CONDITION-FORMAT-ARGUMENTS",
    );
    assert_eq!(runtime.call_builtin(&mut ctx, control, &[direct]), Ok(text));
    assert_eq!(
        runtime.call_builtin(&mut ctx, arguments, &[direct]),
        Ok(value)
    );
}

#[test]
fn cerror_string_datum_with_rest_values_returns_nil_after_unhandled_signal() {
    let (runtime, mut ctx) = setup();
    let cerror = builtin(&runtime, &mut ctx, "COMMON-LISP", "CERROR");
    let text = make_string(
        &mut ctx,
        &runtime,
        &"recover ~a".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            cerror,
            &[Word::NIL, text, Word::fixnum(22), Word::fixnum(33)],
        ),
        Ok(Word::NIL)
    );
    assert!(!ctx.take_non_local_exit());
}

#[test]
fn all_named_restart_shorthands_dispatch_and_missing_names_return_nil() {
    let (runtime, mut ctx) = setup();
    let callback = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    ctx.set_condition_handler_invoker(return_first_argument);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());

    for (name, builtin_name, argument, expected) in [
        (
            "USE-VALUE",
            "USE-VALUE",
            Some(Word::fixnum(1)),
            Word::fixnum(1),
        ),
        (
            "STORE-VALUE",
            "STORE-VALUE",
            Some(Word::fixnum(2)),
            Word::fixnum(2),
        ),
        ("CONTINUE", "CONTINUE", None, Word::NIL),
        ("ABORT", "ABORT", None, Word::NIL),
        ("MUFFLE-WARNING", "MUFFLE-WARNING", None, Word::NIL),
    ] {
        let name_word = make_string(&mut ctx, &runtime, &name.chars().collect::<Vec<_>>()).unwrap();
        let restart = push_restart(
            &mut ctx,
            &runtime,
            name_word,
            callback.as_word(),
            Word::NIL,
            Word::NIL,
            Word::NIL,
        )
        .unwrap();
        let function = builtin(&runtime, &mut ctx, "COMMON-LISP", builtin_name);
        let result = match argument {
            Some(value) => runtime.call_builtin(&mut ctx, function, &[value]),
            None => runtime.call_builtin(&mut ctx, function, &[]),
        };
        assert_eq!(result, Ok(expected));
        pop_restart(&mut ctx, restart);
    }

    let absent = builtin(&runtime, &mut ctx, "COMMON-LISP", "ABORT");
    assert_eq!(runtime.call_builtin(&mut ctx, absent, &[]), Ok(Word::NIL));
}

#[test]
fn handler_registration_and_dispatch_distinguish_unknown_and_parent_classes() {
    let (runtime, mut ctx) = setup();
    let push = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-HANDLER");
    let pop = builtin(&runtime, &mut ctx, "NCL-EXT", "POP-HANDLER");
    let continue_function = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let unknown = symbol(&mut ctx, &runtime, "COMMON-LISP", "W11-NO-CLASS");
    assert_eq!(
        runtime.call_builtin(&mut ctx, push, &[unknown, continue_function.as_word()],),
        Err(ObjectError::Layout)
    );

    let error_symbol = symbol(&mut ctx, &runtime, "COMMON-LISP", "ERROR");
    let error = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, error, &[]).unwrap();
    let token = runtime
        .call_builtin(&mut ctx, push, &[error_symbol, continue_function.as_word()])
        .unwrap();
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    assert_eq!(signal_matched(&mut ctx, condition), Ok(true));
    assert_eq!(runtime.call_builtin(&mut ctx, pop, &[token]), Ok(Word::NIL));
    assert_eq!(signal_matched(&mut ctx, condition), Ok(false));
}

#[test]
fn conversion_covers_nil_arity_and_error_payload_variants() {
    let (runtime, mut ctx) = setup();
    let cases = [
        (
            LispError::ProgramError(ProgramError::WrongNumberOfArguments {
                minimum: 3,
                maximum: None,
            }),
            "PROGRAM-ERROR",
        ),
        (LispError::Object(ObjectError::TypeError), "TYPE-ERROR"),
        (
            LispError::PackageError(PackageError::NotFound),
            "PACKAGE-ERROR",
        ),
        (LispError::StreamError(StreamError::Closed), "STREAM-ERROR"),
        (LispError::EndOfFile, "END-OF-FILE"),
        (LispError::Object(ObjectError::Unbound), "ERROR"),
    ];
    for (error, expected) in cases {
        let condition = condition_from_lisp_error(&mut ctx, &runtime, error).unwrap();
        assert_eq!(class_name(&ctx, condition), expected);
    }

    let arity = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::ProgramError(ProgramError::WrongNumberOfArguments {
            minimum: 3,
            maximum: None,
        }),
    )
    .unwrap();
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(arity), 0),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(arity), 1),
        Ok(Word::NIL)
    );
}

#[test]
fn condition_error_boundary_reports_non_conditions_and_invalid_accessors() {
    let (runtime, mut ctx) = setup();
    let restart_name = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    let find = builtin(&runtime, &mut ctx, "COMMON-LISP", "FIND-RESTART");
    assert_eq!(
        runtime.call_builtin(&mut ctx, restart_name, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[Word::fixnum(1), Word::fixnum(2)]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        condition_class_of(&ctx, Word::NIL),
        Err(ConditionError::NotACondition)
    );

    let cell = condition_class(&mut ctx, &runtime, "CELL-ERROR").unwrap();
    let name = symbol(&mut ctx, &runtime, "COMMON-LISP", "W11-CELL");
    let condition = make_condition(&mut ctx, &runtime, cell, &[name]).unwrap();
    let accessor = builtin(&runtime, &mut ctx, "COMMON-LISP", "CELL-ERROR-NAME");
    assert_eq!(
        runtime.call_builtin(&mut ctx, accessor, &[condition]),
        Ok(name)
    );
}

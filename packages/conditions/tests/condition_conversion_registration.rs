#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Coverage for conversion, registration, class construction, and edge paths.

use ncl_conditions::{
    ConditionError, ConditionIdentifier, ConditionSlotValue, cerror, condition_class,
    condition_class_name, condition_class_of, condition_from_lisp_error, error, make_condition,
    make_condition_record, make_typed_condition, pop_handler, warn,
};
use ncl_object::{
    ArithmeticError, CellError, ControlError, FileError, FunctionObject, LispError, ObjectError,
    ObjectType, Package, PackageError, ProgramError, Runtime, StreamError, ThreadContext, Word,
    make_string, string_length, string_ref,
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
    let name = condition_class_name(ctx, class).unwrap();
    (0..string_length(ctx, name).unwrap())
        .map(|index| string_ref(ctx, name, index).unwrap())
        .collect()
}

fn converted_class(ctx: &ThreadContext, condition: Word) -> String {
    class_name(ctx, condition_class_of(ctx, condition).unwrap())
}

#[test]
fn conversion_maps_typed_lisp_errors_to_specific_conditions() {
    let (runtime, mut ctx) = setup();
    let name = Word::fixnum(17);
    let cases = [
        (
            LispError::TypeError {
                datum: name,
                expected: ObjectType::String,
            },
            "TYPE-ERROR",
        ),
        (
            LispError::ProgramError(ProgramError::UnknownKeyword),
            "PROGRAM-ERROR",
        ),
        (
            LispError::ProgramError(ProgramError::OddKeywordArguments),
            "PROGRAM-ERROR",
        ),
        (
            LispError::ArithmeticError(ArithmeticError::InvalidOperation),
            "ARITHMETIC-ERROR",
        ),
        (
            LispError::ControlError(ControlError::Throw),
            "CONTROL-ERROR",
        ),
        (LispError::CellError(CellError::UnboundSlot), "UNBOUND-SLOT"),
        (
            LispError::CellError(CellError::UnboundVariable { name }),
            "UNBOUND-VARIABLE",
        ),
        (
            LispError::CellError(CellError::UndefinedFunction { name }),
            "UNDEFINED-FUNCTION",
        ),
        (
            LispError::PackageError(PackageError::Conflict),
            "PACKAGE-ERROR",
        ),
        (
            LispError::StreamError(StreamError::InvalidDirection),
            "STREAM-ERROR",
        ),
        (
            LispError::FileError(FileError::PermissionDenied {
                pathname: Word::NIL,
            }),
            "FILE-ERROR",
        ),
        (LispError::Object(ObjectError::Unbound), "ERROR"),
        (
            LispError::Object(ObjectError::Storage(ncl_sys::StorageCondition::InvalidSize)),
            "STORAGE-CONDITION",
        ),
    ];
    for (error, expected) in cases {
        let condition = condition_from_lisp_error(&mut ctx, &runtime, error).unwrap();
        assert_eq!(converted_class(&ctx, condition), expected);
    }
}

#[test]
fn class_construction_and_invalid_condition_designators_are_observable() {
    let (runtime, mut ctx) = setup();
    let class = ConditionIdentifier::ArithmeticError
        .class(&mut ctx, &runtime)
        .unwrap();
    let record = make_condition_record(
        &mut ctx,
        &runtime,
        class,
        &[ConditionSlotValue::from_word(Word::fixnum(1))],
    )
    .unwrap();
    assert_eq!(condition_class_of(&ctx, record.as_word()).unwrap(), class);
    let end_of_file =
        make_typed_condition(&mut ctx, &runtime, ConditionIdentifier::EndOfFile, &[]).unwrap();
    assert_eq!(converted_class(&ctx, end_of_file.as_word()), "END-OF-FILE");
    assert_eq!(
        condition_class_of(&ctx, Word::fixnum(1)),
        Err(ConditionError::NotACondition)
    );
    assert_eq!(
        condition_class_name(
            &ctx,
            ncl_conditions::ConditionClass::from_word(Word::fixnum(1))
        ),
        Err(ConditionError::Object(ObjectError::TypeError))
    );

    let make = builtin(&runtime, &mut ctx, "COMMON-LISP", "MAKE-CONDITION");
    assert_eq!(
        runtime.call_builtin(&mut ctx, make, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn registration_builtins_cover_cell_name_handlers_and_reports() {
    let (runtime, mut ctx) = setup();
    let cell_class = condition_class(&mut ctx, &runtime, "CELL-ERROR").unwrap();
    let cell_name = symbol(&mut ctx, &runtime, "COMMON-LISP", "W5-CELL");
    let cell = make_condition(&mut ctx, &runtime, cell_class, &[cell_name]).unwrap();
    let cell_name_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "CELL-ERROR-NAME");
    assert_eq!(
        runtime.call_builtin(&mut ctx, cell_name_builtin, &[cell]),
        Ok(cell_name)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, cell_name_builtin, &[Word::NIL]),
        Err(ObjectError::TypeError)
    );

    let push = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-HANDLER");
    let pop = builtin(&runtime, &mut ctx, "NCL-EXT", "POP-HANDLER");
    let handler = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let class_name = symbol(&mut ctx, &runtime, "COMMON-LISP", "WARNING");
    let token = runtime
        .call_builtin(&mut ctx, push, &[class_name, handler.as_word()])
        .unwrap();
    assert_eq!(runtime.call_builtin(&mut ctx, pop, &[token]), Ok(Word::NIL));
    assert_eq!(
        runtime.call_builtin(&mut ctx, push, &[Word::fixnum(1), handler.as_word()]),
        Err(ObjectError::TypeError)
    );

    let cerror_builtin = builtin(&runtime, &mut ctx, "COMMON-LISP", "CERROR");
    let message = make_string(
        &mut ctx,
        &runtime,
        &"recoverable".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, cerror_builtin, &[Word::NIL, message]),
        Ok(Word::NIL)
    );
}

#[test]
fn handler_and_restart_edges_clean_up_unhandled_paths() {
    let (runtime, mut ctx) = setup();
    let class = condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    assert_eq!(
        warn(&mut ctx, Word::fixnum(3)),
        Err(ConditionError::NotACondition)
    );
    assert_eq!(
        cerror(&mut ctx, &runtime, Word::NIL, Word::NIL, condition),
        Err(ConditionError::Unhandled)
    );

    let push = builtin(&runtime, &mut ctx, "NCL-EXT", "PUSH-RESTART");
    let pop = builtin(&runtime, &mut ctx, "NCL-EXT", "POP-RESTART");
    let restart_name = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    let token = runtime
        .call_builtin(
            &mut ctx,
            push,
            &[Word::NIL, Word::NIL, Word::NIL, Word::NIL, Word::NIL],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, restart_name, &[token]),
        Ok(Word::NIL)
    );
    assert_eq!(runtime.call_builtin(&mut ctx, pop, &[token]), Ok(Word::NIL));
    pop_handler(
        &mut ctx,
        &runtime,
        ncl_conditions::HandlerChain::from_word(Word::NIL),
    );
    assert_eq!(error(&mut ctx, condition), Err(ConditionError::Unhandled));
    assert!(ctx.take_non_local_exit());
}

#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 18 coverage for the complete standard class identifier matrix and
//! restart/cleanup builtin boundaries.

use ncl_conditions::{
    ConditionError, ConditionIdentifier, condition_class, condition_class_name, condition_class_of,
    make_condition, pop_cleanup, pop_handler, pop_restart, push_cleanup, push_handler,
    push_restart,
};
use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word, make_string};

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

fn all_identifiers() -> &'static [ConditionIdentifier] {
    use ConditionIdentifier::*;
    &[
        Condition, Warning, SeriousCondition, Error, StorageCondition, TypeError,
        SimpleTypeError, ArithmeticError, DivisionByZero, FloatingPointOverflow,
        FloatingPointUnderflow, FloatingPointInvalidOperation, FloatingPointInexact, CellError,
        UnboundVariable, UndefinedFunction, UnboundSlot, FileError, PackageError, ControlError,
        ProgramError, ParseError, ReaderError, PrintNotReadable, StreamError, EndOfFile,
        SimpleCondition, SimpleError, SimpleWarning, StyleWarning, UndefinedAlienError,
        CodeDeletionNote, CompilerNote, DefconstantUneql, DeleteFileError, DeprecationCondition,
        DeprecationError, EarlyDeprecationWarning, FileDoesNotExist, FileExists,
        FinalDeprecationWarning, ImplicitGenericFunctionWarning, InvalidFasl, LateDeprecationWarning,
        NameConflict, PackageDoesNotExist, PackageLockViolation, PackageLockedError,
        ReaderPackageDoesNotExist, StepCondition, StepFinishedCondition, StepFormCondition,
        StepValuesCondition, SymbolPackageLockedError, Timeout, UnknownKeywordArgument,
        SystemCondition, BreakpointError, DeadlineTimeout, InteractiveInterrupt, IoTimeout,
        MemoryFaultError, ThreadError, InterruptThreadError, JoinThreadError,
        SymbolValueInThreadError, ThreadDeadlock,
    ]
}

#[test]
fn every_condition_identifier_resolves_to_a_matching_registered_class() {
    let (runtime, mut ctx) = setup();
    for identifier in all_identifiers() {
        assert_eq!(identifier.name().is_empty(), false);
        let class = identifier.class(&mut ctx, &runtime).unwrap();
        let class_name = condition_class_name(&ctx, class).unwrap();
        let length = ncl_object::string_length(&ctx, class_name).unwrap();
        let actual = (0..length)
            .map(|index| ncl_object::string_ref(&ctx, class_name, index).unwrap())
            .collect::<String>();
        assert_eq!(actual, identifier.name());
        let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
        assert_eq!(condition_class_of(&ctx, condition).unwrap(), class);
    }
}

#[test]
fn compute_restarts_builtin_filters_handlers_and_accepts_ignored_arguments() {
    let (runtime, mut ctx) = setup();
    let compute = builtin(&runtime, &mut ctx, "COMMON-LISP", "COMPUTE-RESTARTS");
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let handler = push_handler(&mut ctx, &runtime, warning, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, compute, &[Word::fixnum(1), Word::fixnum(2)]),
        Ok(Word::NIL)
    );
    let name = make_string(&mut ctx, &runtime, &"W18-RESTART".chars().collect::<Vec<_>>()).unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::fixnum(18),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let list = runtime
        .call_builtin(&mut ctx, compute, &[Word::NIL, Word::fixnum(3)])
        .unwrap();
    assert_ne!(list, Word::NIL);
    assert_eq!(ncl_object::car(&ctx, list).unwrap(), restart.as_word());
    pop_restart(&mut ctx, restart);
    pop_handler(&mut ctx, &runtime, handler);
}

#[test]
fn anonymous_restart_name_and_cleanup_record_boundaries_are_stable() {
    let (runtime, mut ctx) = setup();
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
    let restart_name = builtin(&runtime, &mut ctx, "COMMON-LISP", "RESTART-NAME");
    assert_eq!(
        runtime.call_builtin(&mut ctx, restart_name, &[anonymous.as_word()]),
        Ok(Word::NIL)
    );
    pop_restart(&mut ctx, anonymous);

    let first = push_cleanup(&mut ctx, &runtime, Word::fixnum(1)).unwrap();
    let second = push_cleanup(&mut ctx, &runtime, Word::fixnum(2)).unwrap();
    pop_cleanup(&mut ctx, second);
    pop_cleanup(&mut ctx, first);
    assert!(!ctx.take_non_local_exit());
}

#[test]
fn invalid_class_and_condition_boundaries_preserve_typed_errors() {
    let (runtime, mut ctx) = setup();
    assert_eq!(
        condition_class(&mut ctx, &runtime, "W18-NOT-REGISTERED"),
        None
    );
    let invalid_ctx = ThreadContext::new();
    assert_eq!(
        condition_class_name(
            &invalid_ctx,
            ncl_conditions::ConditionClass::from_word(Word::fixnum(1)),
        ),
        Err(ConditionError::Object(ncl_object::ObjectError::TypeError))
    );
    assert_eq!(
        condition_class_of(&ctx, Word::fixnum(18)),
        Err(ConditionError::NotACondition)
    );
    let _ = symbol(&mut ctx, &runtime, "COMMON-LISP", "W18-SYMBOL");
}

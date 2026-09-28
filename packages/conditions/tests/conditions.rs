#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Behavior tests for the condition system: handler nesting and unwind,
//! restart visibility, non-local exit, the `cerror` continue restart, and
//! type-specific signal dispatch.

use ncl_conditions::{
    ConditionClass, ConditionError, ConditionIdentifier, ConditionSlotValue, cerror,
    compute_restarts, condition_class, error, find_restart, invoke_restart_by_name, make_condition,
    make_typed_condition, pop_handler, pop_restart, push_cleanup, push_handler, push_restart,
    signal, unwind,
};
use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FunctionObject, LispError, ObjectType, Package, Parameter, ParameterType,
    Runtime, ThreadContext, Word, make_string, pop_root, push_root, slot_ref, string_length,
    string_ref, typed_builtin,
};

const fn fail_type_error(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _value: ncl_object::Fixnum,
) -> Result<Word, LispError> {
    Err(LispError::TypeError {
        datum: Word::NIL,
        expected: ObjectType::Fixnum,
    })
}

typed_builtin!(typed_fail_type_error, fail_type_error, (value: ncl_object::Fixnum));

const FAIL_PARAMETERS: &[Parameter] = &[Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Fixnum,
}];

fn collect_during_handler(
    _runtime: std::ptr::NonNull<()>,
    ctx: &mut ThreadContext,
    _handler: Word,
    _condition: Word,
) -> Result<(), ncl_object::ObjectError> {
    ctx.collect(true)
}

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    (runtime, ctx)
}

fn class(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> ConditionClass {
    condition_class(ctx, runtime, name).unwrap()
}

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap()
}

#[test]
fn handler_chain_nests_and_unwinds() {
    let (runtime, mut ctx) = setup();
    let type_error = class(&runtime, &mut ctx, "TYPE-ERROR");
    let program_error = class(&runtime, &mut ctx, "PROGRAM-ERROR");

    let first = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let second = push_handler(&mut ctx, &runtime, program_error, Word::NIL).unwrap();

    let condition = make_condition(&mut ctx, &runtime, type_error, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();

    pop_handler(&mut ctx, &runtime, second);
    pop_handler(&mut ctx, &runtime, first);

    let condition = make_condition(&mut ctx, &runtime, type_error, &[]).unwrap();
    assert_eq!(signal(&mut ctx, condition), Err(ConditionError::Unhandled));
}

#[test]
fn restart_visibility() {
    let (runtime, mut ctx) = setup();
    let name = make_string(
        &mut ctx,
        &runtime,
        &"MY-RESTART".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();

    assert!(find_restart(&ctx, name).unwrap().is_some());
    assert_ne!(compute_restarts(&mut ctx, &runtime).unwrap(), Word::NIL);

    pop_restart(&mut ctx, restart);
    assert!(find_restart(&ctx, name).unwrap().is_none());
}

#[test]
fn error_raises_non_local_exit_when_unhandled() {
    let (runtime, mut ctx) = setup();
    let program_error = class(&runtime, &mut ctx, "PROGRAM-ERROR");
    let condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();

    assert_eq!(error(&mut ctx, condition), Err(ConditionError::Unhandled));
    assert!(ctx.take_non_local_exit());
}

#[test]
fn continue_restart_can_be_invoked() {
    let (runtime, mut ctx) = setup();
    let name = make_string(&mut ctx, &runtime, &"CONTINUE".chars().collect::<Vec<_>>()).unwrap();
    push_restart(
        &mut ctx,
        &runtime,
        name,
        Word::fixnum(42),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();

    assert_eq!(
        invoke_restart_by_name(&mut ctx, name).unwrap(),
        Word::fixnum(42)
    );
    assert!(ctx.take_non_local_exit());
}

#[test]
fn signal_dispatch_is_type_specific() {
    let (runtime, mut ctx) = setup();
    let type_error = class(&runtime, &mut ctx, "TYPE-ERROR");
    let program_error = class(&runtime, &mut ctx, "PROGRAM-ERROR");
    let error_class = class(&runtime, &mut ctx, "ERROR");

    let chain = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, type_error, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();
    pop_handler(&mut ctx, &runtime, chain);

    let chain = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();
    assert_eq!(signal(&mut ctx, condition), Err(ConditionError::Unhandled));
    pop_handler(&mut ctx, &runtime, chain);

    let chain = push_handler(&mut ctx, &runtime, error_class, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();
    pop_handler(&mut ctx, &runtime, chain);
}

#[test]
fn signal_roots_removed_handler_record_during_callback_collection() {
    let (runtime, mut ctx) = setup();
    let class = ncl_conditions::condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    ctx.set_strict_forwarding(true);
    ctx.set_condition_handler_invoker(collect_during_handler);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());

    let chain = push_handler(&mut ctx, &runtime, class, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    ctx.set_gc_stress(true);
    signal(&mut ctx, condition).unwrap();
    let class = ncl_conditions::condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    let condition = make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();
    pop_handler(&mut ctx, &runtime, chain);
}

#[test]
fn unhandled_warning_is_muffled() {
    let (runtime, mut ctx) = setup();
    let warning = class(&runtime, &mut ctx, "WARNING");
    let condition = make_condition(&mut ctx, &runtime, warning, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();
}

#[test]
fn typed_condition_constructor_uses_identifier_hierarchy() {
    let (runtime, mut ctx) = setup();
    let record = make_typed_condition(
        &mut ctx,
        &runtime,
        ConditionIdentifier::TypeError,
        &[ConditionSlotValue::from_word(Word::fixnum(7))],
    )
    .unwrap();

    let class = class(&runtime, &mut ctx, "TYPE-ERROR");
    let chain = push_handler(&mut ctx, &runtime, class, Word::NIL).unwrap();
    signal(&mut ctx, record.as_word()).unwrap();
    pop_handler(&mut ctx, &runtime, chain);
}

#[test]
fn cerror_signals_with_a_continue_restart() {
    let (runtime, mut ctx) = setup();
    let error_class = class(&runtime, &mut ctx, "ERROR");
    let chain = push_handler(&mut ctx, &runtime, error_class, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, error_class, &[]).unwrap();

    cerror(&mut ctx, &runtime, Word::NIL, Word::NIL, condition).unwrap();

    pop_handler(&mut ctx, &runtime, chain);
}

#[test]
fn condition_builtins_accept_rest_and_condition_designators() {
    let (runtime, mut ctx) = setup();
    let program_error = class(&runtime, &mut ctx, "PROGRAM-ERROR");
    let simple_condition = class(&runtime, &mut ctx, "SIMPLE-CONDITION");
    let simple_error = class(&runtime, &mut ctx, "SIMPLE-ERROR");
    let simple_warning = class(&runtime, &mut ctx, "SIMPLE-WARNING");
    let argument =
        make_string(&mut ctx, &runtime, &"argument".chars().collect::<Vec<_>>()).unwrap();
    let message = make_string(&mut ctx, &runtime, &"message".chars().collect::<Vec<_>>()).unwrap();
    let package = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let (program_error_symbol, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "PROGRAM-ERROR")
        .unwrap();

    let signal = builtin(&runtime, &mut ctx, "SIGNAL");
    let descriptor = runtime.builtin_descriptor(signal).unwrap();
    assert_eq!(descriptor.lambda_list.min_arity(), 1);
    assert_eq!(descriptor.lambda_list.max_arity(), None);
    let chain = push_handler(&mut ctx, &runtime, program_error, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            signal,
            &[program_error_symbol, argument, Word::fixnum(7)],
        ),
        Ok(Word::NIL)
    );
    pop_handler(&mut ctx, &runtime, chain);

    let error = builtin(&runtime, &mut ctx, "ERROR");
    let chain = push_handler(&mut ctx, &runtime, simple_error, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, error, &[message, argument]),
        Ok(Word::NIL)
    );
    pop_handler(&mut ctx, &runtime, chain);

    let warn = builtin(&runtime, &mut ctx, "WARN");
    let chain = push_handler(&mut ctx, &runtime, simple_warning, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, warn, &[message, argument]),
        Ok(Word::NIL)
    );
    pop_handler(&mut ctx, &runtime, chain);

    let cerror = builtin(&runtime, &mut ctx, "CERROR");
    let descriptor = runtime.builtin_descriptor(cerror).unwrap();
    assert_eq!(descriptor.lambda_list.min_arity(), 2);
    assert_eq!(descriptor.lambda_list.max_arity(), None);
    let chain = push_handler(&mut ctx, &runtime, simple_error, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            cerror,
            &[message, message, argument, Word::fixnum(7)],
        ),
        Ok(Word::NIL)
    );
    pop_handler(&mut ctx, &runtime, chain);

    let chain = push_handler(&mut ctx, &runtime, simple_condition, Word::NIL).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, signal, &[message, argument]),
        Ok(Word::NIL)
    );
    pop_handler(&mut ctx, &runtime, chain);
}

#[test]
fn unhandled_plain_error_reports_unsupported_after_condition_unwinds() {
    let (runtime, mut ctx) = setup();
    let error = builtin(&runtime, &mut ctx, "ERROR");
    let message =
        make_string(&mut ctx, &runtime, &"unhandled".chars().collect::<Vec<_>>()).unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, error, &[message, Word::fixnum(1)]),
        Err(ncl_object::ObjectError::Unsupported)
    );
}

#[test]
fn unwind_marks_the_exit() {
    let (runtime, mut ctx) = setup();
    push_cleanup(&mut ctx, &runtime, Word::NIL).unwrap();
    unwind(&mut ctx);
    assert!(ctx.take_non_local_exit());
}

#[test]
fn typed_builtin_error_becomes_pending_type_condition() {
    let (runtime, mut ctx) = setup();
    let descriptor = Builtin {
        lambda_list: ncl_object::LambdaList::fixed(FAIL_PARAMETERS),
        convention: BuiltinConvention::Direct(Arity::exact(1)),
    };
    let function = runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("FAIL-TYPE")),
            BuiltinImplementation::direct(descriptor, typed_fail_type_error),
        )
        .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let mut condition = ctx.take_pending_condition().unwrap();
    let condition_token = push_root(&mut ctx, &mut condition);
    let class = ncl_conditions::condition_class_of(&ctx, condition).unwrap();
    let class_name = ncl_conditions::condition_class_name(&ctx, class).unwrap();
    assert_eq!(string_length(&ctx, class_name).unwrap(), 10);
    for (index, character) in "TYPE-ERROR".chars().enumerate() {
        assert_eq!(string_ref(&ctx, class_name, index).unwrap(), character);
    }
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(condition), 0).unwrap(),
        Word::NIL
    );
    let package = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let (expected_type, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "FIXNUM")
        .unwrap();
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(condition), 1).unwrap(),
        expected_type
    );
    pop_root(&mut ctx, condition_token);
}

#[test]
fn undefined_function_condition_preserves_name_and_accessor_returns_it() {
    let (runtime, mut ctx) = setup();
    let package = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let (name, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "MISSING-FUNCTION")
        .unwrap();
    let condition = ncl_conditions::condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::CellError(ncl_object::CellError::UndefinedFunction { name }),
    )
    .unwrap();
    let class = ncl_conditions::condition_class_of(&ctx, condition).unwrap();
    assert_eq!(
        class,
        ncl_conditions::ConditionIdentifier::UndefinedFunction
            .class(&mut ctx, &runtime)
            .unwrap()
    );
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "CELL-ERROR-NAME")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[condition]),
        Ok(name)
    );
}

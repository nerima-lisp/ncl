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
    Builtin, BuiltinImplementation, BuiltinPackage, BuiltinIdentifier, BuiltinName,
    BuiltinConvention, Arity, LispError, ObjectType, Parameter, ParameterType, Runtime,
    Package, ThreadContext, Word, make_string, pop_root, push_root, slot_ref, string_length,
    string_ref, typed_builtin,
};

fn fail_type_error(
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

#[test]
fn handler_chain_nests_and_unwinds() {
    let (runtime, mut ctx) = setup();
    let type_error = class(&runtime, &mut ctx, "TYPE-ERROR");
    let program_error = class(&runtime, &mut ctx, "PROGRAM-ERROR");

    let first = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let second = push_handler(&mut ctx, &runtime, program_error, Word::NIL).unwrap();

    let condition = make_condition(&mut ctx, &runtime, type_error, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();

    pop_handler(&mut ctx, second);
    pop_handler(&mut ctx, first);

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
    pop_handler(&mut ctx, chain);

    let chain = push_handler(&mut ctx, &runtime, type_error, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();
    assert_eq!(signal(&mut ctx, condition), Err(ConditionError::Unhandled));
    pop_handler(&mut ctx, chain);

    let chain = push_handler(&mut ctx, &runtime, error_class, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, program_error, &[]).unwrap();
    signal(&mut ctx, condition).unwrap();
    pop_handler(&mut ctx, chain);
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
    pop_handler(&mut ctx, chain);
}

#[test]
fn cerror_signals_with_a_continue_restart() {
    let (runtime, mut ctx) = setup();
    let error_class = class(&runtime, &mut ctx, "ERROR");
    let chain = push_handler(&mut ctx, &runtime, error_class, Word::NIL).unwrap();
    let condition = make_condition(&mut ctx, &runtime, error_class, &[]).unwrap();

    cerror(&mut ctx, &runtime, Word::NIL, Word::NIL, condition).unwrap();

    pop_handler(&mut ctx, chain);
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

    assert_eq!(runtime.call_builtin(&mut ctx, function, &[Word::NIL]), Err(ncl_object::ObjectError::TypeError));
    let mut condition = ctx.take_pending_condition().unwrap();
    let condition_token = push_root(&mut ctx, &mut condition);
    let class = ncl_conditions::condition_class_of(&ctx, condition).unwrap();
    let class_name = ncl_conditions::condition_class_name(&ctx, class).unwrap();
    assert_eq!(string_length(&ctx, class_name).unwrap(), 10);
    for (index, character) in "TYPE-ERROR".chars().enumerate() {
        assert_eq!(string_ref(&ctx, class_name, index).unwrap(), character);
    }
    assert_eq!(slot_ref(&ctx, ncl_object::Instance::from_word(condition), 0).unwrap(), Word::NIL);
    let package = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let (expected_type, _) = Package::from_word(package).intern(&mut ctx, &runtime, "FIXNUM").unwrap();
    assert_eq!(slot_ref(&ctx, ncl_object::Instance::from_word(condition), 1).unwrap(), expected_type);
    pop_root(&mut ctx, condition_token);
}

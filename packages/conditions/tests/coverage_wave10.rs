#![allow(clippy::unnecessary_wraps)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on condition-system behavior"
)]

//! Wave 10 coverage for record traversal, restart shorthand, class edges, and
//! conversion payloads.

use ncl_conditions::{
    ConditionIdentifier, condition_class, condition_class_name, condition_class_of,
    condition_from_lisp_error, make_condition, pop_handler, pop_restart, push_handler,
    push_restart, signal,
};
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LispError, ObjectError, ObjectType, Parameter,
    ParameterType, ProgramError, Runtime, ThreadContext, Word, make_string, slot_ref,
    string_length, string_ref,
};

const ONE_VALUE: &[Parameter] = &[Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
}];

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

fn text(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).unwrap())
        .map(|index| string_ref(ctx, value, index).unwrap())
        .collect()
}

fn symbol_text(ctx: &ThreadContext, value: Word) -> String {
    text(ctx, ncl_object::symbol_name(ctx, value).unwrap())
}

fn return_first_argument(
    _runtime: std::ptr::NonNull<()>,
    _ctx: &mut ThreadContext,
    _function: Word,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    Ok(arguments.first().copied().unwrap_or(Word::NIL))
}

fn forward_value(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    args.required(0)
}

#[test]
fn handler_search_skips_restart_records_and_preserves_them() {
    let (runtime, mut ctx) = setup();
    let warning = condition_class(&mut ctx, &runtime, "WARNING").unwrap();
    let condition = make_condition(&mut ctx, &runtime, warning, &[]).unwrap();
    let restart_name = make_string(
        &mut ctx,
        &runtime,
        &"W10-RESTART".chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        restart_name,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    let handler_function = builtin(&runtime, &mut ctx, "COMMON-LISP", "CONTINUE");
    let handler = push_handler(&mut ctx, &runtime, warning, handler_function.as_word()).unwrap();
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());

    assert_eq!(signal(&mut ctx, condition), Ok(()));
    assert_eq!(
        ncl_conditions::find_restart(&ctx, restart_name),
        Ok(Some(restart.as_word()))
    );

    pop_handler(&mut ctx, &runtime, handler);
    pop_restart(&mut ctx, restart);
    assert_eq!(ncl_conditions::find_restart(&ctx, restart_name), Ok(None));
}

#[test]
fn restart_shorthand_invokes_callable_and_returns_its_argument() {
    let (runtime, mut ctx) = setup();
    let descriptor = Builtin {
        lambda_list: ncl_object::LambdaList::fixed(ONE_VALUE),
        convention: BuiltinConvention::Direct(Arity::exact(1)),
    };
    let callback = runtime
        .register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("W10-FORWARD")),
            BuiltinImplementation::direct(descriptor, forward_value),
        )
        .unwrap();
    let name = make_string(&mut ctx, &runtime, &"USE-VALUE".chars().collect::<Vec<_>>()).unwrap();
    let restart = push_restart(
        &mut ctx,
        &runtime,
        name,
        callback.as_word(),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap();
    assert_eq!(
        ncl_conditions::find_restart(&ctx, name),
        Ok(Some(restart.as_word()))
    );
    ctx.set_condition_handler_invoker(return_first_argument);
    ctx.set_evaluator_runtime(std::ptr::NonNull::<()>::dangling().as_ptr());
    let use_value = builtin(&runtime, &mut ctx, "COMMON-LISP", "USE-VALUE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, use_value, &[Word::fixnum(101)]),
        Ok(Word::fixnum(101))
    );
    pop_restart(&mut ctx, restart);
    assert_eq!(
        runtime.call_builtin(&mut ctx, use_value, &[Word::fixnum(202)]),
        Ok(Word::NIL)
    );
}

#[test]
fn class_lookup_and_names_cover_identifier_and_mismatch_edges() {
    let (runtime, mut ctx) = setup();
    let root = condition_class(&mut ctx, &runtime, "CONDITION").unwrap();
    let child = condition_class(&mut ctx, &runtime, "TYPE-ERROR").unwrap();
    assert_eq!(
        text(&ctx, condition_class_name(&ctx, root).unwrap()),
        "CONDITION"
    );
    assert_eq!(
        text(&ctx, condition_class_name(&ctx, child).unwrap()),
        "TYPE-ERROR"
    );
    assert_ne!(
        text(&ctx, condition_class_name(&ctx, child).unwrap()),
        "CONDITION"
    );
    assert_eq!(condition_class(&mut ctx, &runtime, "W10-MISSING"), None);
    let record = make_condition(&mut ctx, &runtime, child, &[]).unwrap();
    assert_eq!(condition_class_of(&ctx, record).unwrap(), child);
}

#[test]
fn conversion_keeps_type_and_arity_payload_slots() {
    let (runtime, mut ctx) = setup();
    let datum = Word::fixnum(17);
    let type_error = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::TypeError {
            datum,
            expected: ObjectType::String,
        },
    )
    .unwrap();
    let expected = slot_ref(&ctx, ncl_object::Instance::from_word(type_error), 1).unwrap();
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(type_error), 0),
        Ok(datum)
    );
    assert_eq!(symbol_text(&ctx, expected), "STRING");

    let program_error = condition_from_lisp_error(
        &mut ctx,
        &runtime,
        LispError::ProgramError(ProgramError::WrongNumberOfArguments {
            minimum: 2,
            maximum: Some(4),
        }),
    )
    .unwrap();
    assert_eq!(
        condition_class_of(&ctx, program_error).unwrap(),
        ConditionIdentifier::ProgramError
            .class(&mut ctx, &runtime)
            .unwrap()
    );
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(program_error), 0),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        slot_ref(&ctx, ncl_object::Instance::from_word(program_error), 1),
        Ok(Word::fixnum(4))
    );
}

use super::*;

#[allow(clippy::unwrap_used, reason = "coverage tests assert on initialization results")]
fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context registration: {error:?}"));
    (runtime, ctx)
}

#[test]
#[allow(clippy::unwrap_used, reason = "coverage tests assert on initialization results")]
fn initargs_parse_even_pairs_and_reject_odd_input() {
    let parsed = InitArgList::parse(&[
        Word::fixnum(1),
        Word::TRUE,
        Word::fixnum(2),
        Word::NIL,
    ])
    .unwrap_or_else(|error| panic!("even initargs: {error:?}"));
    assert_eq!(parsed.value_for(Word::fixnum(1)).map(|value| value.0), Some(Word::TRUE));
    assert!(parsed.value_for(Word::fixnum(9)).is_none());
    assert!(matches!(
        InitArgList::parse(&[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    ));

    let even_values = [Word::fixnum(0), Word::fixnum(1), Word::TRUE, Word::fixnum(2), Word::NIL];
    let args = BuiltinArgs::new(&even_values);
    assert_eq!(initarg_adapter(&args), Ok(args.as_slice().to_vec()));
    let odd_values = [Word::fixnum(0), Word::fixnum(1), Word::TRUE, Word::fixnum(2)];
    let odd = BuiltinArgs::new(&odd_values);
    assert_eq!(initarg_adapter(&odd), Err(ObjectError::TypeError));
}

#[test]
#[allow(clippy::unwrap_used, reason = "coverage tests assert on initialization results")]
fn class_slots_and_class_resolution_cover_vector_shapes() {
    let (runtime, mut ctx) = context();
    let short = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL, Word::NIL])
        .unwrap_or_else(|error| panic!("short class: {error:?}"));
    assert_eq!(class_slots(&ctx, short), Ok(Vec::new()));
    assert_eq!(class_slots(&ctx, Word::fixnum(0)), Err(ObjectError::TypeError));

    let first = Word::fixnum(11);
    let descriptor = ncl_object::make_simple_vector(&mut ctx, &runtime, &[first])
        .unwrap_or_else(|error| panic!("descriptor: {error:?}"));
    let long = ncl_object::make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::NIL, Word::NIL, Word::NIL, Word::NIL, descriptor],
    )
    .unwrap_or_else(|error| panic!("long class: {error:?}"));
    assert_eq!(class_slots(&ctx, long), Ok(vec![first]));

    assert_eq!(resolve_class(&mut ctx, &runtime, long), Ok(long));
    assert_eq!(resolve_class(&mut ctx, &runtime, Word::fixnum(0)), Err(ObjectError::TypeError));
    let unknown = Package::from_word(runtime.find_package(&ctx, "COMMON-LISP").unwrap())
        .intern(&mut ctx, &runtime, "NO-SUCH-CLOS-CLASS")
        .unwrap()
        .0;
    assert_eq!(resolve_class(&mut ctx, &runtime, unknown), Err(ObjectError::TypeError));
}

#[test]
#[allow(clippy::unwrap_used, reason = "coverage tests assert on initialization results")]
fn initialization_callbacks_report_missing_follow_up_functions() {
    let (runtime, mut ctx) = context();
    let class = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL, Word::NIL])
        .unwrap_or_else(|error| panic!("class: {error:?}"));
    let mut values = MultipleValues::default();
    assert_eq!(
        make_instance_builtin(&mut ctx, &runtime, &BuiltinArgs::new(&[class]), &mut values),
        Err(ObjectError::UndefinedFunction)
    );

    let instance = allocate_instance(&mut ctx, &runtime, class, &[Word::UNBOUND])
        .unwrap_or_else(|error| panic!("instance: {error:?}"));
    assert_eq!(
        initialize_instance_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[instance.as_word()]),
            &mut values,
        ),
        Err(ObjectError::UndefinedFunction)
    );
    assert_eq!(
        shared_initialize_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[instance.as_word()]),
            &mut values,
        ),
        Ok(instance.as_word())
    );
}

#[test]
#[allow(clippy::unwrap_used, reason = "coverage tests assert on initialization results")]
fn shared_initialize_applies_descriptor_defaults_and_returns_instance() {
    let (runtime, mut ctx) = context();
    let key = Word::fixnum(31);
    let default = Word::fixnum(32);
    let slot = ncl_object::make_simple_vector(&mut ctx, &runtime, &[key, Word::NIL, default])
        .unwrap_or_else(|error| panic!("slot descriptor: {error:?}"));
    let slots = ncl_object::make_simple_vector(&mut ctx, &runtime, &[slot])
        .unwrap_or_else(|error| panic!("slots: {error:?}"));
    let class = ncl_object::make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL, slots])
        .unwrap_or_else(|error| panic!("class: {error:?}"));
    let instance = allocate_instance(&mut ctx, &runtime, class, &[Word::UNBOUND])
        .unwrap_or_else(|error| panic!("instance: {error:?}"));
    let mut values = MultipleValues::default();
    assert_eq!(
        shared_initialize_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[instance.as_word()]),
            &mut values,
        ),
        Ok(instance.as_word())
    );
    assert_eq!(ncl_object::slot_ref(&ctx, instance, 0), Ok(default));
}

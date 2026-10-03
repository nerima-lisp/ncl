use super::*;
use ncl_object::{Runtime, ThreadContext};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

static PLACE_EXPANSIONS: AtomicUsize = AtomicUsize::new(0);
static PLACE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn place_expander(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    PLACE_EXPANSIONS.fetch_add(1, Ordering::Relaxed);
    let value = arguments.first().copied().ok_or(ObjectError::TypeError)?;
    let temporary = fresh_symbol(ctx, runtime)?;
    let store = fresh_symbol(ctx, runtime)?;
    let set = symbol(ctx, runtime, "SET")?;
    Ok(SetfExpansion {
        temporary_variables: vec![temporary],
        value_forms: vec![value],
        store_variables: vec![store],
        store_form: list(ctx, runtime, &[set, store, temporary])?,
        access_form: temporary,
    })
}

fn place_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: Word,
    value: Word,
) -> Result<Word, ObjectError> {
    list(ctx, runtime, &[operator, value])
}

fn head(ctx: &mut ThreadContext, form: Word) -> Result<Word, ObjectError> {
    elements(ctx, form)?
        .first()
        .copied()
        .ok_or(ObjectError::TypeError)
}

fn named(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    form: Word,
    name: &str,
) -> Result<(), ObjectError> {
    assert_eq!(head(ctx, form)?, symbol(ctx, runtime, name)?); // check-added-lines: allow(panic) test assertion
    Ok(())
}

fn malformed_expander(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    arguments: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    arguments
        .first()
        .ok_or(ObjectError::TypeError)
        .map(|_| SetfExpansion {
            temporary_variables: vec![Word::fixnum(1)],
            value_forms: Vec::new(),
            store_variables: Vec::new(),
            store_form: Word::NIL,
            access_form: Word::NIL,
        })
}

fn expect_type_error<T: std::fmt::Debug>(
    result: &Result<T, ObjectError>,
) -> Result<(), ObjectError> {
    if matches!(result, Err(ObjectError::TypeError)) {
        Ok(())
    } else {
        Err(ObjectError::TypeError)
    }
}

#[test]
fn place_subforms_are_expanded_once_per_place() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "TEST-PLACE")?;
    let side_effect = symbol(&mut ctx, &runtime, "SIDE-EFFECT")?;
    let registry = PlaceRegistry::new(&runtime);
    registry.define(&ctx, operator, place_expander)?;
    let first = place_form(&mut ctx, &runtime, operator, side_effect)?;
    let second = place_form(&mut ctx, &runtime, operator, side_effect)?;

    PLACE_EXPANSIONS.store(0, Ordering::Relaxed);
    expand_psetf(&mut ctx, &runtime, &registry, &[first, Word::fixnum(1)])?;
    assert_eq!(PLACE_EXPANSIONS.load(Ordering::Relaxed), 1);

    PLACE_EXPANSIONS.store(0, Ordering::Relaxed);
    expand_shiftf(
        &mut ctx,
        &runtime,
        &registry,
        &[first, second, Word::fixnum(1)],
    )?;
    assert_eq!(PLACE_EXPANSIONS.load(Ordering::Relaxed), 2);

    PLACE_EXPANSIONS.store(0, Ordering::Relaxed);
    expand_rotatef(&mut ctx, &runtime, &registry, &[first, second])?;
    assert_eq!(PLACE_EXPANSIONS.load(Ordering::Relaxed), 2);

    for expand in [
        expand_incf
            as fn(
                &mut ThreadContext,
                &Runtime,
                &PlaceRegistry,
                &[Word],
            ) -> Result<Word, ObjectError>,
        expand_decf,
    ] {
        PLACE_EXPANSIONS.store(0, Ordering::Relaxed);
        expand(&mut ctx, &runtime, &registry, &[first])?;
        assert_eq!(PLACE_EXPANSIONS.load(Ordering::Relaxed), 1);
    }

    for new_only in [false, true] {
        PLACE_EXPANSIONS.store(0, Ordering::Relaxed);
        expand_push(
            &mut ctx,
            &runtime,
            &registry,
            &[Word::fixnum(1), first],
            new_only,
        )?;
        assert_eq!(PLACE_EXPANSIONS.load(Ordering::Relaxed), 1);
    }

    PLACE_EXPANSIONS.store(0, Ordering::Relaxed);
    expand_pop(&mut ctx, &runtime, &registry, &[first])?;
    assert_eq!(PLACE_EXPANSIONS.load(Ordering::Relaxed), 1);
    Ok(())
}

#[test]
fn psetf_keeps_each_place_and_value_in_source_order() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "ORDERED-PLACE")?;
    let registry = PlaceRegistry::new(&runtime);
    registry.define(&ctx, operator, place_expander)?;
    let first = place_form(&mut ctx, &runtime, operator, Word::fixnum(11))?;
    let second = place_form(&mut ctx, &runtime, operator, Word::fixnum(22))?;

    let expansion = expand_psetf(
        &mut ctx,
        &runtime,
        &registry,
        &[first, Word::fixnum(1), second, Word::fixnum(2)],
    )?;
    let outer = elements(&mut ctx, expansion)?;
    assert_eq!(outer.len(), 3);
    assert_eq!(outer[0], symbol(&mut ctx, &runtime, "LET*")?);
    let bindings = elements(&mut ctx, outer[1])?;
    assert_eq!(bindings.len(), 4);
    assert_eq!(elements(&mut ctx, bindings[0])?[1], Word::fixnum(11));
    assert_eq!(elements(&mut ctx, bindings[1])?[1], Word::fixnum(1));
    assert_eq!(elements(&mut ctx, bindings[2])?[1], Word::fixnum(22));
    assert_eq!(elements(&mut ctx, bindings[3])?[1], Word::fixnum(2));
    Ok(())
}

#[test]
fn modifying_macros_emit_the_expected_value_and_store_forms() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let x = symbol(&mut ctx, &runtime, "X")?;
    let one = Word::fixnum(1);

    for (expand, operator) in [
        (
            expand_incf
                as fn(
                    &mut ThreadContext,
                    &Runtime,
                    &PlaceRegistry,
                    &[Word],
                ) -> Result<Word, ObjectError>,
            "+",
        ),
        (expand_decf, "-"),
    ] {
        let expansion = expand(&mut ctx, &runtime, &registry, &[x, one])?;
        let outer = elements(&mut ctx, expansion)?;
        named(&mut ctx, &runtime, expansion, "LET")?;
        let binding = elements(&mut ctx, outer[1])?; // check-added-lines: allow(index) fixed expansion shape
        let binding = binding.first().copied().ok_or(ObjectError::TypeError)?;
        let binding = elements(&mut ctx, binding)?;
        let arithmetic = elements(&mut ctx, binding[1])?; // check-added-lines: allow(index) fixed expansion shape
        assert_eq!(arithmetic[0], symbol(&mut ctx, &runtime, operator)?); // check-added-lines: allow(panic,index) test assertion
        assert_eq!(arithmetic[1], x); // check-added-lines: allow(panic,index) test assertion
        assert_eq!(arithmetic[2], one); // check-added-lines: allow(panic,index) test assertion
        named(&mut ctx, &runtime, outer[2], "SETQ")?;
    }

    let push = expand_push(&mut ctx, &runtime, &registry, &[one, x], false)?;
    let push_body = elements(&mut ctx, push)?[2]; // check-added-lines: allow(index) fixed expansion shape
    named(&mut ctx, &runtime, push_body, "SETQ")?;
    let push_parts = elements(&mut ctx, push)?;
    let push_binding = elements(&mut ctx, push_parts[1])?[0]; // check-added-lines: allow(index) fixed expansion shape
    let push_binding_parts = elements(&mut ctx, push_binding)?;
    let cons = head(&mut ctx, push_binding_parts[1])?; // check-added-lines: allow(index) fixed expansion shape
    assert_eq!(cons, symbol(&mut ctx, &runtime, "CONS")?); // check-added-lines: allow(panic) test assertion

    let test = symbol(&mut ctx, &runtime, "TEST")?;
    let pushnew = expand_push(&mut ctx, &runtime, &registry, &[one, x, test], true)?;
    let pushnew_parts = elements(&mut ctx, pushnew)?;
    let pushnew_binding = elements(&mut ctx, pushnew_parts[1])?[0]; // check-added-lines: allow(index) fixed expansion shape
    let pushnew_binding_parts = elements(&mut ctx, pushnew_binding)?;
    let adjoin = head(&mut ctx, pushnew_binding_parts[1])?; // check-added-lines: allow(index) fixed expansion shape
    assert_eq!(adjoin, symbol(&mut ctx, &runtime, "ADJOIN")?); // check-added-lines: allow(panic) test assertion

    let pop = expand_pop(&mut ctx, &runtime, &registry, &[x])?;
    let pop_outer = elements(&mut ctx, pop)?;
    named(&mut ctx, &runtime, pop, "LET*")?;
    let pop_let = elements(&mut ctx, pop_outer[2])?; // check-added-lines: allow(index) fixed expansion shape
    named(&mut ctx, &runtime, pop_outer[2], "LET")?; // check-added-lines: allow(index) fixed expansion shape
    let pop_body = elements(&mut ctx, pop_let[2])?; // check-added-lines: allow(index) fixed expansion shape
    named(&mut ctx, &runtime, pop_let[2], "PROGN")?; // check-added-lines: allow(index) fixed expansion shape
    assert_eq!(pop_body.len(), 3); // check-added-lines: allow(panic) test assertion
    named(&mut ctx, &runtime, pop_body[2], "CAR")?; // check-added-lines: allow(index) fixed expansion shape
    Ok(())
}

#[test]
fn modifying_macros_accept_the_wrapped_place() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let the = symbol(&mut ctx, &runtime, "THE")?;
    let fixnum = symbol(&mut ctx, &runtime, "FIXNUM")?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let typed_place = list(&mut ctx, &runtime, &[the, fixnum, x])?;

    let expansion = expand_incf(&mut ctx, &runtime, &registry, &[typed_place])?;
    let parts = elements(&mut ctx, expansion)?;
    let binding = elements(&mut ctx, *parts.get(1).ok_or(ObjectError::TypeError)?)?
        .first()
        .copied()
        .ok_or(ObjectError::TypeError)?;
    let binding_parts = elements(&mut ctx, binding)?;
    let arithmetic = elements(
        &mut ctx,
        *binding_parts.get(1).ok_or(ObjectError::TypeError)?,
    )?;
    assert_eq!(*arithmetic.get(1).ok_or(ObjectError::TypeError)?, x);
    named(
        &mut ctx,
        &runtime,
        *parts.get(2).ok_or(ObjectError::TypeError)?,
        "SETQ",
    )?;
    Ok(())
}

#[test]
fn remf_and_expansion_support_validate_their_error_paths() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let x = symbol(&mut ctx, &runtime, "X")?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;

    let remf = expand_remf(&mut ctx, &runtime, &registry, &[x, key])?;
    let remf_parts = elements(&mut ctx, remf)?;
    let remf_body = elements(&mut ctx, remf_parts[2])?; // check-added-lines: allow(index) fixed expansion shape
    named(&mut ctx, &runtime, remf_parts[2], "IF")?; // check-added-lines: allow(index) fixed expansion shape
    assert_eq!(remf_body.len(), 4); // check-added-lines: allow(panic) test assertion
    assert_eq!(remf_body[3], Word::NIL); // check-added-lines: allow(panic,index) test assertion

    expect_type_error(&expand_incf(&mut ctx, &runtime, &registry, &[]))?;
    expect_type_error(&expand_decf(&mut ctx, &runtime, &registry, &[]))?;
    expect_type_error(&expand_push(&mut ctx, &runtime, &registry, &[x], false))?;
    expand_push(&mut ctx, &runtime, &registry, &[x, x], false)?;
    expect_type_error(&expand_pop(&mut ctx, &runtime, &registry, &[]))?;
    expect_type_error(&expand_remf(&mut ctx, &runtime, &registry, &[x]))?;
    expect_type_error(&expand_shiftf(&mut ctx, &runtime, &registry, &[x]))?;
    expect_type_error(&expand_rotatef(&mut ctx, &runtime, &registry, &[]))?;

    let bad_operator = symbol(&mut ctx, &runtime, "BAD-PLACE")?;
    registry.define(&ctx, bad_operator, malformed_expander)?;
    let bad_place = place_form(&mut ctx, &runtime, bad_operator, x)?;
    expect_type_error(&expand_incf(&mut ctx, &runtime, &registry, &[bad_place]))?;
    expect_type_error(&crate::setf_support::with_expansion_roots(
        &mut ctx,
        &SetfExpansion {
            temporary_variables: vec![x],
            value_forms: Vec::new(),
            store_variables: Vec::new(),
            store_form: Word::NIL,
            access_form: Word::NIL,
        },
        |_ctx, _, _, _, _, _| Ok(()),
    ))?;
    Ok(())
}

#[test]
fn get_setf_expansion_returns_all_five_expansion_values() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let place = symbol(&mut ctx, &runtime, "PLACE")?;
    let mut values = ncl_object::MultipleValues::new();

    let result = crate::get_setf_expansion_callback(&runtime, &mut ctx, &[place], &mut values)?;

    assert_eq!(result, place); // check-added-lines: allow(panic) exact callback result assertion.
    assert_eq!(values.len(), 5); // check-added-lines: allow(panic) exact expansion count assertion.
    let returned = values.as_slice();
    assert_eq!(returned[0], Word::NIL); // check-added-lines: allow(panic,index) exact expansion assertion.
    assert_eq!(returned[1], Word::NIL); // check-added-lines: allow(panic,index) exact expansion assertion.
    let store_variables = elements(&mut ctx, returned[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(store_variables.len(), 1); // check-added-lines: allow(panic) exact expansion count assertion.
    let store_form = elements(&mut ctx, returned[3])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(store_form[0], symbol(&mut ctx, &runtime, "SETQ")?); // check-added-lines: allow(panic,index) exact expansion assertion.
    assert_eq!(store_form[1], place); // check-added-lines: allow(panic,index) exact expansion assertion.
    assert_eq!(store_form[2], store_variables[0]); // check-added-lines: allow(panic,index) exact expansion assertion.
    assert_eq!(returned[4], place); // check-added-lines: allow(panic,index) exact expansion assertion.
    Ok(())
}

#[test]
fn expansion_support_roots_values_and_propagates_callback_errors() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let temporary = symbol(&mut ctx, &runtime, "TEMPORARY")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let store = symbol(&mut ctx, &runtime, "STORE")?;
    let store_form = symbol(&mut ctx, &runtime, "STORE-FORM")?;
    let access = symbol(&mut ctx, &runtime, "ACCESS")?;
    let expansion = SetfExpansion {
        temporary_variables: vec![temporary],
        value_forms: vec![value],
        store_variables: vec![store],
        store_form,
        access_form: access,
    };

    let observed: Result<(), ObjectError> = crate::setf_support::with_expansion_roots(
        &mut ctx,
        &expansion,
        |_ctx, temporaries, values, stores, actual_store_form, actual_access| {
            assert_eq!(temporaries, &[temporary]); // check-added-lines: allow(panic) callback contract assertion.
            assert_eq!(values, &[value]); // check-added-lines: allow(panic) callback contract assertion.
            assert_eq!(stores, &[store]); // check-added-lines: allow(panic) callback contract assertion.
            assert_eq!(actual_store_form, store_form); // check-added-lines: allow(panic) callback contract assertion.
            assert_eq!(actual_access, access); // check-added-lines: allow(panic) callback contract assertion.
            Err(ObjectError::UndefinedFunction)
        },
    );
    assert_eq!(observed, Err(ObjectError::UndefinedFunction)); // check-added-lines: allow(panic) exact error propagation assertion.

    let expansion_values = crate::setf_support::expansion_values(&mut ctx, &runtime, &expansion)?;
    let temporary_values = elements(&mut ctx, expansion_values[0])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let value_forms = elements(&mut ctx, expansion_values[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let store_values = elements(&mut ctx, expansion_values[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(temporary_values, vec![temporary]); // check-added-lines: allow(panic) exact expansion assertion.
    assert_eq!(value_forms, vec![value]); // check-added-lines: allow(panic) exact expansion assertion.
    assert_eq!(store_values, vec![store]); // check-added-lines: allow(panic) exact expansion assertion.
    assert_eq!(expansion_values[3], store_form); // check-added-lines: allow(panic,index) exact expansion assertion.
    assert_eq!(expansion_values[4], access); // check-added-lines: allow(panic,index) exact expansion assertion.
    Ok(())
}

#[test]
fn setf_expansion_support_rejects_invalid_shapes_and_arguments() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "PLACE")?;
    let operator = symbol(&mut ctx, &runtime, "INVALID-PLACE")?;
    registry.define(&ctx, operator, malformed_expander)?;
    let malformed_place = place_form(&mut ctx, &runtime, operator, place)?;

    let empty_setf = expand_setf(&mut ctx, &runtime, &registry, &[])?;
    named(&mut ctx, &runtime, empty_setf, "PROGN")?;
    expect_type_error(&expand_psetf(&mut ctx, &runtime, &registry, &[place]))?;
    expect_type_error(&expand_setf(
        &mut ctx,
        &runtime,
        &registry,
        &[malformed_place, Word::fixnum(1)],
    ))?;
    let symbol_place_expansion =
        expand_setf(&mut ctx, &runtime, &registry, &[place, Word::fixnum(1)])?;
    named(&mut ctx, &runtime, symbol_place_expansion, "PROGN")?;

    let invalid_store_count = SetfExpansion {
        temporary_variables: Vec::new(),
        value_forms: Vec::new(),
        store_variables: vec![place, operator],
        store_form: Word::NIL,
        access_form: place,
    };
    expect_type_error(&crate::setf_support::with_expansion_roots(
        &mut ctx,
        &invalid_store_count,
        |_ctx, _, _, _, _, _| Ok(()),
    ))?;
    Ok(())
}

#[test]
fn setf_arity_and_registry_boundaries_return_the_exact_error() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "PLACE")?;
    let cases = vec![
        ("INCF", Vec::new()),
        ("DECF", Vec::new()),
        ("PUSH", vec![place]),
        ("POP", Vec::new()),
        ("REMF", vec![place]),
        ("SHIFTF", vec![place]),
        ("ROTATEF", Vec::new()),
    ];
    for (name, arguments) in cases {
        let result = match name {
            "INCF" => expand_incf(&mut ctx, &runtime, &registry, &arguments),
            "DECF" => expand_decf(&mut ctx, &runtime, &registry, &arguments),
            "PUSH" => expand_push(&mut ctx, &runtime, &registry, &arguments, false),
            "POP" => expand_pop(&mut ctx, &runtime, &registry, &arguments),
            "REMF" => expand_remf(&mut ctx, &runtime, &registry, &arguments),
            "SHIFTF" => expand_shiftf(&mut ctx, &runtime, &registry, &arguments),
            "ROTATEF" => expand_rotatef(&mut ctx, &runtime, &registry, &arguments),
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) exhaustive table uses only named operators.
        };
        // check-added-lines: allow(panic) exact error assertion.
        assert_eq!(result, Err(ObjectError::TypeError));
    }

    let other_runtime = Runtime::new()?;
    let mut other_ctx = ThreadContext::new();
    other_ctx.register(&other_runtime)?;
    let other_place = symbol(&mut other_ctx, &other_runtime, "PLACE")?;
    // check-added-lines: allow(panic) exact registry error assertion.
    assert_eq!(
        expand_setf(
            &mut other_ctx,
            &other_runtime,
            &registry,
            &[other_place, other_place]
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn setf_rotation_matrix_preserves_sources_and_emits_complete_shapes() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let first = symbol(&mut ctx, &runtime, "FIRST")?;
    let second = symbol(&mut ctx, &runtime, "SECOND")?;
    let replacement = symbol(&mut ctx, &runtime, "REPLACEMENT")?;
    let cases = vec![
        (
            expand_shiftf
                as fn(
                    &mut ThreadContext,
                    &Runtime,
                    &PlaceRegistry,
                    &[Word],
                ) -> Result<Word, ObjectError>,
            vec![first, second, replacement],
        ),
        (
            expand_rotatef
                as fn(
                    &mut ThreadContext,
                    &Runtime,
                    &PlaceRegistry,
                    &[Word],
                ) -> Result<Word, ObjectError>,
            vec![first, second],
        ),
    ];
    for (expand, arguments) in cases {
        let expanded = expand(&mut ctx, &runtime, &registry, &arguments)?;
        let outer = elements(&mut ctx, expanded)?;
        assert_eq!(outer[0], symbol(&mut ctx, &runtime, "LET*")?); // check-added-lines: allow(panic,index) exact expansion assertion.
        let bindings = elements(&mut ctx, outer[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
        assert_eq!(bindings.len(), 2); // check-added-lines: allow(panic) exact expansion count assertion.
        let body = elements(&mut ctx, outer[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
        assert_eq!(body[0], symbol(&mut ctx, &runtime, "PROGN")?); // check-added-lines: allow(panic,index) exact expansion assertion.
        assert_eq!(body.len(), 4); // check-added-lines: allow(panic) exact expansion count assertion.
        let first_binding = elements(&mut ctx, bindings[0])?; // check-added-lines: allow(index) expansion shape is asserted below.
        assert_eq!(body.last().copied(), first_binding.first().copied()); // check-added-lines: allow(panic) exact expansion assertion.
    }
    Ok(())
}

#[test]
fn setf_expanders_cover_default_arguments_and_complete_error_contracts() -> Result<(), ObjectError>
{
    type Expander =
        fn(&mut ThreadContext, &Runtime, &PlaceRegistry, &[Word]) -> Result<Word, ObjectError>;

    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "PLACE")?;

    let default_cases = [
        (
            expand_incf
                as fn(
                    &mut ThreadContext,
                    &Runtime,
                    &PlaceRegistry,
                    &[Word],
                ) -> Result<Word, ObjectError>,
            "+",
        ),
        (expand_decf, "-"),
    ];
    for (expand, operator) in default_cases {
        let expanded = expand(&mut ctx, &runtime, &registry, &[place])?;
        let outer = elements(&mut ctx, expanded)?;
        // check-added-lines: allow(panic,index) exact expansion assertion.
        assert_eq!(outer[0], symbol(&mut ctx, &runtime, "LET")?);
        let bindings = elements(&mut ctx, outer[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
        let binding = elements(&mut ctx, bindings[0])?; // check-added-lines: allow(index) expansion shape is asserted below.
        let arithmetic = elements(&mut ctx, binding[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
        // check-added-lines: allow(panic) exact arithmetic expansion assertion.
        assert_eq!(
            arithmetic,
            vec![
                symbol(&mut ctx, &runtime, operator)?,
                place,
                Word::fixnum(1)
            ]
        );
    }

    let error_cases: [(&str, Expander, Vec<Word>); 7] = [
        ("SETF", expand_setf, vec![place]),
        ("PSETF", expand_psetf, vec![place]),
        ("INCF", expand_incf, Vec::new()),
        ("DECF", expand_decf, Vec::new()),
        ("SHIFTF", expand_shiftf, vec![place]),
        ("ROTATEF", expand_rotatef, Vec::new()),
        ("REMF", expand_remf, vec![place]),
    ];
    for (name, expand, arguments) in error_cases {
        // check-added-lines: allow(panic) exact malformed argument assertion.
        assert_eq!(
            expand(&mut ctx, &runtime, &registry, &arguments),
            Err(ObjectError::TypeError),
            "{name} malformed arguments"
        );
    }

    let unknown_operator = symbol(&mut ctx, &runtime, "UNKNOWN-PLACE")?;
    let unknown = list(&mut ctx, &runtime, &[unknown_operator, place])?;
    // check-added-lines: allow(panic) exact unknown place assertion.
    assert_eq!(
        expand_setf(&mut ctx, &runtime, &registry, &[unknown, place]),
        Err(ObjectError::UndefinedFunction)
    );

    let other_runtime = Runtime::new()?;
    let other_registry = PlaceRegistry::new(&other_runtime);
    // check-added-lines: allow(panic) exact registry ownership assertion.
    assert_eq!(
        expand_setf(
            &mut ctx,
            &runtime,
            &other_registry,
            &[place, Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn pushnew_and_rotate_expansions_preserve_their_complete_forms() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let place = symbol(&mut ctx, &runtime, "PLACE")?;
    let item = symbol(&mut ctx, &runtime, "ITEM")?;
    let test = symbol(&mut ctx, &runtime, "TEST")?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let pushnew = expand_push(
        &mut ctx,
        &runtime,
        &registry,
        &[item, place, test, key],
        true,
    )?;
    let pushnew_parts = elements(&mut ctx, pushnew)?;
    let pushnew_bindings = elements(&mut ctx, pushnew_parts[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let pushnew_binding = elements(&mut ctx, pushnew_bindings[0])?; // check-added-lines: allow(index) expansion shape is asserted below.
    let adjoin = elements(&mut ctx, pushnew_binding[1])?; // check-added-lines: allow(index) expansion shape is asserted below.
    // check-added-lines: allow(panic) exact expansion assertion.
    assert_eq!(
        adjoin,
        vec![
            symbol(&mut ctx, &runtime, "ADJOIN")?,
            item,
            place,
            test,
            key,
        ]
    );

    let left = symbol(&mut ctx, &runtime, "LEFT")?;
    let right = symbol(&mut ctx, &runtime, "RIGHT")?;
    let shifted = expand_shiftf(
        &mut ctx,
        &runtime,
        &registry,
        &[left, right, Word::fixnum(9)],
    )?;
    let shifted_parts = elements(&mut ctx, shifted)?;
    assert_eq!(shifted_parts[0], symbol(&mut ctx, &runtime, "LET*")?); // check-added-lines: allow(panic,index) exact expansion assertion.
    let shifted_body = elements(&mut ctx, shifted_parts[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(shifted_body[0], symbol(&mut ctx, &runtime, "PROGN")?); // check-added-lines: allow(panic,index) exact expansion assertion.
    assert_eq!(shifted_body.len(), 4); // check-added-lines: allow(panic) exact expansion count assertion.

    let rotated = expand_rotatef(&mut ctx, &runtime, &registry, &[left, right])?;
    let rotated_parts = elements(&mut ctx, rotated)?;
    assert_eq!(rotated_parts[0], symbol(&mut ctx, &runtime, "LET*")?); // check-added-lines: allow(panic,index) exact expansion assertion.
    let rotated_body = elements(&mut ctx, rotated_parts[2])?; // check-added-lines: allow(index) expansion shape is asserted below.
    assert_eq!(rotated_body[0], symbol(&mut ctx, &runtime, "PROGN")?); // check-added-lines: allow(panic,index) exact expansion assertion.
    assert_eq!(rotated_body.len(), 4); // check-added-lines: allow(panic) exact expansion count assertion.
    Ok(())
}

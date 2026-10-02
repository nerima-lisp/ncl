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

fn expect_type_error<T>(result: Result<T, ObjectError>) -> Result<(), ObjectError> {
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

    expect_type_error(expand_incf(&mut ctx, &runtime, &registry, &[]))?;
    expect_type_error(expand_decf(&mut ctx, &runtime, &registry, &[]))?;
    expect_type_error(expand_push(&mut ctx, &runtime, &registry, &[x], false))?;
    expand_push(&mut ctx, &runtime, &registry, &[x, x], false)?;
    expect_type_error(expand_pop(&mut ctx, &runtime, &registry, &[]))?;
    expect_type_error(expand_remf(&mut ctx, &runtime, &registry, &[x]))?;
    expect_type_error(expand_shiftf(&mut ctx, &runtime, &registry, &[x]))?;
    expect_type_error(expand_rotatef(&mut ctx, &runtime, &registry, &[]))?;

    let bad_operator = symbol(&mut ctx, &runtime, "BAD-PLACE")?;
    registry.define(&ctx, bad_operator, malformed_expander)?;
    let bad_place = place_form(&mut ctx, &runtime, bad_operator, x)?;
    expect_type_error(expand_incf(&mut ctx, &runtime, &registry, &[bad_place]))?;
    expect_type_error(crate::setf_support::with_expansion_roots(
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

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

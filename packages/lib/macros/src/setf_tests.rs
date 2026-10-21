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
    ncl_object::with_roots(ctx, arguments, |ctx, arguments| {
        let value = arguments.first().ok_or(ObjectError::TypeError)?.get();
        let temporary = fresh_symbol(ctx, runtime)?;
        let temporary = ncl_object::HeapRootedWord::new(ctx, temporary)?;
        let store = fresh_symbol(ctx, runtime)?;
        let store = ncl_object::HeapRootedWord::new(ctx, store)?;
        let set = symbol(ctx, runtime, "SET")?;
        let set = ncl_object::HeapRootedWord::new(ctx, set)?;
        let store_form = list(ctx, runtime, &[set.get(), store.get(), temporary.get()])?;
        let store_form = ncl_object::HeapRootedWord::new(ctx, store_form)?;
        Ok(SetfExpansion {
            temporary_variables: vec![temporary.get()],
            value_forms: vec![value],
            store_variables: vec![store.get()],
            store_form: store_form.get(),
            access_form: temporary.get(),
        })
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
    assert_eq!(
        elements(&mut ctx, outer[2])?.last().copied(),
        Some(Word::NIL)
    );
    Ok(())
}

#[test]
fn setf_construction_survives_moving_gc_between_places() -> Result<(), ObjectError> {
    let _guard = PLACE_TEST_LOCK.lock().map_err(|_| ObjectError::TypeError)?;
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ctx.set_strict_forwarding(true);
    let operator = symbol(&mut ctx, &runtime, "MOVING-PLACE")?;
    let registry = PlaceRegistry::new(&runtime);
    registry.define(&ctx, operator, place_expander)?;
    let first = place_form(&mut ctx, &runtime, operator, Word::fixnum(11))?;
    let second = place_form(&mut ctx, &runtime, operator, Word::fixnum(22))?;
    ctx.set_gc_stress(true);

    ncl_object::with_heap_rooted_slice(&mut ctx, &[first, second], |ctx, places| {
        let form = expand_setf(
            ctx,
            &runtime,
            &registry,
            &[
                places.first().ok_or(ObjectError::TypeError)?.get(),
                Word::fixnum(1),
                places.get(1).ok_or(ObjectError::TypeError)?.get(),
                Word::fixnum(2),
            ],
        )?;
        let form = ncl_object::HeapRootedWord::new(ctx, form)?;
        ctx.collect(true)?;
        assert!(!elements(ctx, form.get())?.is_empty());

        let form = expand_psetf(
            ctx,
            &runtime,
            &registry,
            &[
                places.first().ok_or(ObjectError::TypeError)?.get(),
                Word::fixnum(3),
                places.get(1).ok_or(ObjectError::TypeError)?.get(),
                Word::fixnum(4),
            ],
        )?;
        let form = ncl_object::HeapRootedWord::new(ctx, form)?;
        ctx.collect(true)?;
        assert!(!elements(ctx, form.get())?.is_empty());
        Ok(())
    })
}

#[test]
fn defsetf_vector_quasiquote_roots_elements_during_allocation() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    ctx.set_strict_forwarding(true);

    let operator = symbol(&mut ctx, &runtime, "VECTOR-DATA")?;
    let datum = list(&mut ctx, &runtime, &[operator, Word::fixnum(31)])?;
    let mut scope = ncl_object::Scope::new(&mut ctx);
    let datum = scope.root::<Word>(ncl_object::Local::<Word>::from_word(datum));
    let datum_word = scope.get(datum).as_word();
    let handles = scope.root_many::<Word>(&[ncl_object::Local::<Word>::from_word(datum_word)]);
    let vector = scope.make_simple_vector(&runtime, &handles)?;
    scope.context_mut().set_gc_stress(true);

    let environment = MacroEnvironment::new();
    let expanded =
        super::defsetf::quasiquote_vector(&mut scope, &runtime, vector, &environment, 1)?;
    let expected = scope.get(datum).as_word();
    let expanded_word = scope.get(expanded).as_word();
    assert_eq!(
        ncl_object::simple_vector_length(scope.context(), expanded_word)?,
        1
    );
    let actual = ncl_object::simple_vector_ref(scope.context(), expanded_word, 0)?;
    assert_eq!(
        elements(scope.context_mut(), actual)?,
        elements(scope.context_mut(), expected)?
    );
    Ok(())
}

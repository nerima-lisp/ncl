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

fn invalid_place_expander(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    arguments: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    let value = arguments.first().copied().ok_or(ObjectError::TypeError)?;
    Ok(SetfExpansion {
        temporary_variables: Vec::new(),
        value_forms: vec![value],
        store_variables: vec![Word::NIL],
        store_form: Word::NIL,
        access_form: value,
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

fn scope_list<'scope>(
    scope: &mut ncl_object::Scope<'scope>,
    runtime: &Runtime,
    parts: &[ncl_object::Handle<'scope, Word>],
) -> Result<ncl_object::Handle<'scope, Word>, ObjectError> {
    let words = parts
        .iter()
        .map(|part| scope.get(*part).as_word())
        .collect::<Vec<_>>();
    let form = list(scope.context_mut(), runtime, &words)?;
    Ok(scope.root(ncl_object::Local::from_word(form)))
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
fn setf_forms_validate_pairs_and_empty_psetf_expands_to_progn() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);

    assert!(matches!(
        expand_setf(&mut ctx, &runtime, &registry, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    ));
    assert!(matches!(
        expand_psetf(&mut ctx, &runtime, &registry, &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    ));

    let expansion = expand_psetf(&mut ctx, &runtime, &registry, &[])?;
    let parts = elements(&mut ctx, expansion)?;
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], symbol(&mut ctx, &runtime, "PROGN")?);
    assert_eq!(parts[1], Word::NIL);
    Ok(())
}

#[test]
fn place_value_preserves_the_type_chain_and_rejects_malformed_the() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let the = symbol(&mut ctx, &runtime, "THE")?;
    let fixnum = symbol(&mut ctx, &runtime, "FIXNUM")?;
    let integer = symbol(&mut ctx, &runtime, "INTEGER")?;
    let variable = symbol(&mut ctx, &runtime, "VALUE")?;
    let inner = list(&mut ctx, &runtime, &[the, integer, variable])?;
    let place = list(&mut ctx, &runtime, &[the, fixnum, inner])?;
    let replacement = symbol(&mut ctx, &runtime, "REPLACEMENT")?;

    let value = super::place_value(&mut ctx, &runtime, place, replacement)?;
    let outer_parts = elements(&mut ctx, value)?;
    assert_eq!(outer_parts.len(), 3);
    assert_eq!(outer_parts[0], the);
    assert_eq!(outer_parts[1], integer);
    let inner_parts = elements(&mut ctx, outer_parts[2])?;
    assert_eq!(inner_parts, vec![the, fixnum, replacement]);

    let malformed = list(&mut ctx, &runtime, &[the, fixnum])?;
    assert!(matches!(
        super::place_value(&mut ctx, &runtime, malformed, replacement),
        Err(ObjectError::TypeError)
    ));

    let registry = PlaceRegistry::new(&runtime);
    assert!(matches!(
        expand_get_setf_expansion(&mut ctx, &runtime, &registry, Word::fixnum(1)),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}

#[test]
fn place_rejects_a_foreign_registry_malformed_the_and_invalid_expansion() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let other_runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let registry = PlaceRegistry::new(&runtime);
    let value = symbol(&mut ctx, &runtime, "PLACE-VALUE")?;

    assert!(matches!(
        expand_get_setf_expansion(
            &mut ctx,
            &runtime,
            &PlaceRegistry::new(&other_runtime),
            value,
        ),
        Err(ObjectError::TypeError)
    ));

    let the = symbol(&mut ctx, &runtime, "THE")?;
    let fixnum = symbol(&mut ctx, &runtime, "FIXNUM")?;
    let malformed_the = list(&mut ctx, &runtime, &[the, fixnum, value, Word::NIL])?;
    assert!(matches!(
        expand_get_setf_expansion(&mut ctx, &runtime, &registry, malformed_the),
        Err(ObjectError::TypeError)
    ));

    let operator = symbol(&mut ctx, &runtime, "INVALID-PLACE-EXPANSION")?;
    registry.define(&ctx, operator, invalid_place_expander)?;
    let invalid_place = place_form(&mut ctx, &runtime, operator, value)?;
    assert!(matches!(
        expand_get_setf_expansion(&mut ctx, &runtime, &registry, invalid_place),
        Err(ObjectError::TypeError)
    ));
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

#[test]
fn quasiquote_handles_nested_unquote_list_splicing_and_top_level_error() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let mut scope = ncl_object::Scope::new(&mut ctx);
    let unquote = scope.intern(&runtime, "COMMON-LISP", "UNQUOTE")?;
    let unquote_splicing = scope.intern(&runtime, "COMMON-LISP", "UNQUOTE-SPLICING")?;
    let quasiquote = scope.intern(&runtime, "COMMON-LISP", "QUASIQUOTE")?;
    let variable = scope.intern(&runtime, "NCL", "QQ-VALUE")?;
    let splice_variable = scope.intern(&runtime, "NCL", "QQ-SPLICE")?;
    let value = scope.root(ncl_object::Local::from_word(Word::fixnum(42)));
    let first = scope.intern(&runtime, "NCL", "QQ-FIRST")?;
    let second = scope.intern(&runtime, "NCL", "QQ-SECOND")?;
    let tail = scope.intern(&runtime, "NCL", "QQ-TAIL")?;
    let splice_values = scope_list(&mut scope, &runtime, &[first, second])?;
    let environment = vec![(variable, value), (splice_variable, splice_values)];

    let unquote_form = scope_list(&mut scope, &runtime, &[unquote, variable])?;
    let value_form = super::eval_quasiquote(&mut scope, &runtime, unquote_form, &environment, 1)?;
    assert_eq!(scope.get(value_form).as_word(), Word::fixnum(42));

    let vector_item_word = scope.get(unquote_form).as_word();
    let vector_word =
        ncl_object::make_simple_vector(scope.context_mut(), &runtime, &[vector_item_word])?;
    let vector = scope.root(ncl_object::Local::from_word(vector_word));
    let vector_result = super::eval_quasiquote(&mut scope, &runtime, vector, &environment, 1)?;
    let vector_word = scope.get(vector_result).as_word();
    assert_eq!(
        ncl_object::simple_vector_length(scope.context(), vector_word)?,
        1
    );
    assert_eq!(
        ncl_object::simple_vector_ref(scope.context(), vector_word, 0)?,
        Word::fixnum(42)
    );

    let nested_unquote = scope_list(&mut scope, &runtime, &[quasiquote, unquote_form])?;
    let nested_result =
        super::eval_quasiquote(&mut scope, &runtime, nested_unquote, &environment, 1)?;
    let nested_result_word = scope.get(nested_result).as_word();
    let nested_parts = elements(scope.context_mut(), nested_result_word)?;
    assert_eq!(nested_parts.len(), 2);
    assert_eq!(nested_parts[0], scope.get(quasiquote).as_word());
    let unquote_parts = elements(scope.context_mut(), nested_parts[1])?;
    assert_eq!(
        unquote_parts,
        vec![scope.get(unquote).as_word(), scope.get(variable).as_word()]
    );

    let nested_splice = scope_list(&mut scope, &runtime, &[unquote_splicing, splice_variable])?;
    let nested_splice_form = scope_list(&mut scope, &runtime, &[quasiquote, nested_splice])?;
    let nested_splice_result =
        super::eval_quasiquote(&mut scope, &runtime, nested_splice_form, &environment, 1)?;
    let nested_splice_result_word = scope.get(nested_splice_result).as_word();
    let nested_splice_parts = elements(scope.context_mut(), nested_splice_result_word)?;
    assert_eq!(nested_splice_parts.len(), 2);
    assert_eq!(nested_splice_parts[0], scope.get(quasiquote).as_word());
    let preserved_splice = elements(scope.context_mut(), nested_splice_parts[1])?;
    assert_eq!(
        preserved_splice,
        vec![
            scope.get(unquote_splicing).as_word(),
            scope.get(splice_variable).as_word(),
        ]
    );

    let list_splice = scope_list(&mut scope, &runtime, &[unquote_splicing, splice_variable])?;
    let spliced_datum = scope_list(&mut scope, &runtime, &[list_splice, tail])?;
    let spliced_result =
        super::eval_quasiquote(&mut scope, &runtime, spliced_datum, &environment, 1)?;
    let spliced_result_word = scope.get(spliced_result).as_word();
    let spliced_elements = elements(scope.context_mut(), spliced_result_word)?;
    assert_eq!(
        spliced_elements,
        vec![
            scope.get(first).as_word(),
            scope.get(second).as_word(),
            scope.get(tail).as_word(),
        ]
    );

    assert!(matches!(
        super::eval_quasiquote(&mut scope, &runtime, list_splice, &environment, 1),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}

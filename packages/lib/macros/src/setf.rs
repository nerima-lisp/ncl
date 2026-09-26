#![allow(missing_docs, clippy::missing_errors_doc)]
#![allow(clippy::trivially_copy_pass_by_ref, clippy::chunks_exact_to_as_chunks)]

use crate::{elements, fresh_symbol, list, symbol, PlaceRegistry, SetfExpansion};
use ncl_object::{classify_object, ObjectError, ObjectRef, Runtime, ThreadContext, Word};

fn form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let mut values = Vec::with_capacity(args.len() + 1);
    values.push(symbol(ctx, runtime, name)?);
    values.extend_from_slice(args);
    list(ctx, runtime, &values)
}

fn binding(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    variable: Word,
    value: Word,
) -> Result<Word, ObjectError> {
    list(ctx, runtime, &[variable, value])
}

fn wrap_let(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    bindings: &[Word],
    body: Word,
) -> Result<Word, ObjectError> {
    let bindings = list(ctx, runtime, bindings)?;
    form(ctx, runtime, name, &[bindings, body])
}

fn sequence(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    forms: &[Word],
) -> Result<Word, ObjectError> {
    form(ctx, runtime, "PROGN", forms)
}

fn place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    place: Word,
) -> Result<SetfExpansion, ObjectError> {
    if !registry.belongs_to(runtime) {
        return Err(ObjectError::TypeError);
    }
    if matches!(classify_object(ctx, place), ObjectRef::Symbol(_)) {
        let set = symbol(ctx, runtime, "SET")?;
        let store = fresh_symbol(ctx, runtime)?;
        let store_form = list(ctx, runtime, &[set, place, store])?;
        return Ok(SetfExpansion {
            temporary_variables: Vec::new(),
            value_forms: Vec::new(),
            store_variables: vec![store],
            store_form,
            access_form: place,
        });
    }
    if !place.is_cons() {
        return Err(ObjectError::TypeError);
    }
    let parts = elements(ctx, place)?;
    let (operator, arguments) = parts.split_first().ok_or(ObjectError::TypeError)?;
    let expansion = registry
        .get(ctx, *operator)?
        .ok_or(ObjectError::UndefinedFunction)?(ctx, runtime, arguments)?;
    validate_expansion(&expansion)?;
    Ok(expansion)
}

fn validate_expansion(expansion: &SetfExpansion) -> Result<(), ObjectError> {
    if expansion.temporary_variables.len() != expansion.value_forms.len()
        || expansion.store_variables.len() != 1
    {
        return Err(ObjectError::TypeError);
    }
    Ok(())
}

fn store_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: &SetfExpansion,
    value: Word,
) -> Result<Word, ObjectError> {
    validate_expansion(expansion)?;
    let store_binding = binding(ctx, runtime, expansion.store_variables[0], value)?;
    let body = wrap_let(ctx, runtime, "LET", &[store_binding], expansion.store_form)?;
    let mut bindings = Vec::with_capacity(expansion.temporary_variables.len());
    for (variable, value_form) in expansion
        .temporary_variables
        .iter()
        .copied()
        .zip(expansion.value_forms.iter().copied())
    {
        bindings.push(binding(ctx, runtime, variable, value_form)?);
    }
    if bindings.is_empty() {
        Ok(body)
    } else {
        wrap_let(ctx, runtime, "LET*", &bindings, body)
    }
}

fn setf_pairs(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
    parallel: bool,
) -> Result<Word, ObjectError> {
    if !arguments.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    if parallel {
        let mut evaluation_bindings = Vec::new();
        let mut stores = Vec::new();
        for pair in arguments.chunks_exact(2) {
            let expansion = place(ctx, runtime, registry, pair[0])?;
            for (variable, value_form) in expansion
                .temporary_variables
                .iter()
                .copied()
                .zip(expansion.value_forms.iter().copied())
            {
                evaluation_bindings.push(binding(ctx, runtime, variable, value_form)?);
            }
            if expansion.store_variables.len() != 1 {
                return Err(ObjectError::TypeError);
            }
            let value_variable = fresh_symbol(ctx, runtime)?;
            evaluation_bindings.push(binding(ctx, runtime, value_variable, pair[1])?);
            stores.push((expansion, value_variable));
        }
        let mut store_forms = Vec::new();
        for (expansion, value_variable) in stores {
            store_forms.push(store_place(ctx, runtime, &expansion, value_variable)?);
        }
        let body = sequence(ctx, runtime, &store_forms)?;
        if evaluation_bindings.is_empty() {
            Ok(body)
        } else {
            wrap_let(ctx, runtime, "LET*", &evaluation_bindings, body)
        }
    } else {
        let mut forms = Vec::new();
        for pair in arguments.chunks_exact(2) {
            let expansion = place(ctx, runtime, registry, pair[0])?;
            forms.push(store_place(ctx, runtime, &expansion, pair[1])?);
        }
        sequence(ctx, runtime, &forms)
    }
}

pub fn expand_setf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    setf_pairs(ctx, runtime, registry, arguments, false)
}

pub fn expand_psetf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    setf_pairs(ctx, runtime, registry, arguments, true)
}

fn modify(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
    operator: &str,
) -> Result<Word, ObjectError> {
    let place_word = *arguments.first().ok_or(ObjectError::TypeError)?;
    let delta = arguments.get(1).copied().unwrap_or(Word::fixnum(1));
    let expansion = place(ctx, runtime, registry, place_word)?;
    let arithmetic = form(ctx, runtime, operator, &[expansion.access_form, delta])?;
    store_place(ctx, runtime, &expansion, arithmetic)
}

pub fn expand_incf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    modify(ctx, runtime, registry, arguments, "+")
}

pub fn expand_decf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    modify(ctx, runtime, registry, arguments, "-")
}

pub fn expand_push(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
    new_only: bool,
) -> Result<Word, ObjectError> {
    let item = *arguments.first().ok_or(ObjectError::TypeError)?;
    let place_word = *arguments.get(1).ok_or(ObjectError::TypeError)?;
    let expansion = place(ctx, runtime, registry, place_word)?;
    let operator = if new_only { "ADJOIN" } else { "CONS" };
    let mut values = vec![item, expansion.access_form];
    if new_only {
        values.extend_from_slice(&arguments[2..]);
    }
    let value = form(ctx, runtime, operator, &values)?;
    store_place(ctx, runtime, &expansion, value)
}

pub fn expand_pop(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    let place_word = *arguments.first().ok_or(ObjectError::TypeError)?;
    let expansion = place(ctx, runtime, registry, place_word)?;
    let old = fresh_symbol(ctx, runtime)?;
    let car = form(ctx, runtime, "CAR", &[old])?;
    let cdr = form(ctx, runtime, "CDR", &[old])?;
    let store = store_place(ctx, runtime, &expansion, cdr)?;
    let body = sequence(ctx, runtime, &[store, car])?;
    let old_binding = binding(ctx, runtime, old, expansion.access_form)?;
    let body = wrap_let(ctx, runtime, "LET", &[old_binding], body)?;
    wrap_let(ctx, runtime, "LET*", &[], body)
}

pub fn expand_shiftf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    if arguments.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    rotate_like(ctx, runtime, registry, arguments, false)
}

pub fn expand_rotatef(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    if arguments.is_empty() {
        return Err(ObjectError::TypeError);
    }
    rotate_like(ctx, runtime, registry, arguments, true)
}

fn rotate_like(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
    rotate: bool,
) -> Result<Word, ObjectError> {
    let place_count = if rotate {
        arguments.len()
    } else {
        arguments.len() - 1
    };
    let mut expansions = Vec::new();
    let mut bindings = Vec::new();
    let mut old_values = Vec::new();
    for place_word in &arguments[..place_count] {
        let expansion = place(ctx, runtime, registry, *place_word)?;
        let old = fresh_symbol(ctx, runtime)?;
        for (variable, value_form) in expansion
            .temporary_variables
            .iter()
            .copied()
            .zip(expansion.value_forms.iter().copied())
        {
            bindings.push(binding(ctx, runtime, variable, value_form)?);
        }
        bindings.push(binding(ctx, runtime, old, expansion.access_form)?);
        expansions.push(expansion);
        old_values.push(old);
    }
    let mut stores = Vec::new();
    for index in 0..place_count {
        let source = if index + 1 < place_count {
            old_values[index + 1]
        } else if rotate {
            old_values[0]
        } else {
            arguments[place_count]
        };
        stores.push(store_place(ctx, runtime, &expansions[index], source)?);
    }
    stores.push(old_values[0]);
    let body = sequence(ctx, runtime, &stores)?;
    wrap_let(ctx, runtime, "LET*", &bindings, body)
}

pub fn expand_remf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    if arguments.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    let expansion = place(ctx, runtime, registry, arguments[0])?;
    let property_list = fresh_symbol(ctx, runtime)?;
    let removed = form(ctx, runtime, "REMF", &[property_list, arguments[1]])?;
    let store = store_place(ctx, runtime, &expansion, property_list)?;
    let body = form(ctx, runtime, "IF", &[removed, store, Word::NIL])?;
    let property_binding = binding(ctx, runtime, property_list, expansion.access_form)?;
    wrap_let(ctx, runtime, "LET", &[property_binding], body)
}

pub fn expand_get_setf_expansion(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    place_word: Word,
) -> Result<Vec<Word>, ObjectError> {
    let expansion = place(ctx, runtime, registry, place_word)?;
    Ok(vec![
        list(ctx, runtime, &expansion.temporary_variables)?,
        list(ctx, runtime, &expansion.value_forms)?,
        list(ctx, runtime, &expansion.store_variables)?,
        expansion.store_form,
        expansion.access_form,
    ])
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use ncl_object::{Runtime, ThreadContext};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

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
        let _guard = PLACE_TEST_LOCK.lock().unwrap();
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
        let _guard = PLACE_TEST_LOCK.lock().unwrap();
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
}

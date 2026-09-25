#![allow(missing_docs, clippy::missing_errors_doc)]

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
    registry.get(*operator).ok_or(ObjectError::Unsupported)?(ctx, runtime, arguments)
}

fn store_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: &SetfExpansion,
    value: Word,
) -> Result<Word, ObjectError> {
    if expansion.store_variables.len() != 1 {
        return Err(ObjectError::Unsupported);
    }
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
        let mut location_bindings = Vec::new();
        let mut value_bindings = Vec::new();
        let mut stores = Vec::new();
        for pair in arguments.chunks_exact(2) {
            let expansion = place(ctx, runtime, registry, pair[0])?;
            let mut location = Vec::new();
            for (variable, value_form) in expansion
                .temporary_variables
                .iter()
                .copied()
                .zip(expansion.value_forms.iter().copied())
            {
                location.push(binding(ctx, runtime, variable, value_form)?);
            }
            location_bindings.extend(location);
            if expansion.store_variables.len() != 1 {
                return Err(ObjectError::Unsupported);
            }
            let value_variable = fresh_symbol(ctx, runtime)?;
            value_bindings.push(binding(ctx, runtime, value_variable, pair[1])?);
            stores.push((expansion, value_variable));
        }
        let mut store_forms = Vec::new();
        for (expansion, value_variable) in stores {
            store_forms.push(store_place(ctx, runtime, &expansion, value_variable)?);
        }
        let body = sequence(ctx, runtime, &store_forms)?;
        let body = wrap_let(ctx, runtime, "LET*", &value_bindings, body)?;
        if location_bindings.is_empty() {
            Ok(body)
        } else {
            wrap_let(ctx, runtime, "LET*", &location_bindings, body)
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

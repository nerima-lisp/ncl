#![allow(missing_docs, clippy::missing_errors_doc)]
#![allow(clippy::trivially_copy_pass_by_ref, clippy::chunks_exact_to_as_chunks)]

use crate::{PlaceRegistry, SetfExpansion, elements, fresh_symbol, list, symbol};
use ncl_object::{ObjectError, ObjectRef, Runtime, ThreadContext, Word, classify_object};

fn form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut operator = symbol(ctx, runtime, name)?;
        ncl_object::with_root(ctx, &mut operator, |ctx, operator| {
            let mut values = Vec::with_capacity(roots.len() + 1);
            values.push(*operator);
            values.extend(roots.iter().map(|value| **value));
            list(ctx, runtime, &values)
        })
    })
}

fn binding(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    variable: Word,
    value: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, &[variable, value], |ctx, roots| {
        list(
            ctx,
            runtime,
            &roots.iter().map(|root| **root).collect::<Vec<_>>(),
        )
    })
}

fn wrap_let(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    bindings: &[Word],
    body: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, bindings, |ctx, roots| {
        ncl_object::with_root(ctx, &mut body.clone(), |ctx, body| {
            let bindings = list(
                ctx,
                runtime,
                &roots.iter().map(|root| **root).collect::<Vec<_>>(),
            )?;
            form(ctx, runtime, name, &[bindings, *body])
        })
    })
}

use crate::setf_support::with_expansion_roots;
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
        let mut set = symbol(ctx, runtime, "SET")?;
        return ncl_object::with_root(ctx, &mut set, |ctx, set| {
            let store = fresh_symbol(ctx, runtime)?;
            let store_form = list(ctx, runtime, &[*set, place, store])?;
            Ok(SetfExpansion {
                temporary_variables: Vec::new(),
                value_forms: Vec::new(),
                store_variables: vec![store],
                store_form,
                access_form: place,
            })
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

const fn validate_expansion(expansion: &SetfExpansion) -> Result<(), ObjectError> {
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
    with_expansion_roots(
        ctx,
        expansion,
        |ctx, temporary, values, stores, store_form, _| {
            let store_variable = *stores.first().ok_or(ObjectError::TypeError)?;
            let store_binding = binding(ctx, runtime, store_variable, value)?;
            let body = wrap_let(ctx, runtime, "LET", &[store_binding], store_form)?;
            let mut bindings = Vec::with_capacity(temporary.len());
            for (variable, value_form) in temporary.iter().copied().zip(values.iter().copied()) {
                bindings.push(binding(ctx, runtime, variable, value_form)?);
            }
            if bindings.is_empty() {
                Ok(body)
            } else {
                wrap_let(ctx, runtime, "LET*", &bindings, body)
            }
        },
    )
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
            let place_word = pair.first().copied().ok_or(ObjectError::TypeError)?;
            let value = pair.get(1).copied().ok_or(ObjectError::TypeError)?;
            let expansion = place(ctx, runtime, registry, place_word)?;
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
            evaluation_bindings.push(binding(ctx, runtime, value_variable, value)?);
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
            let place_word = pair.first().copied().ok_or(ObjectError::TypeError)?;
            let value = pair.get(1).copied().ok_or(ObjectError::TypeError)?;
            let expansion = place(ctx, runtime, registry, place_word)?;
            forms.push(store_place(ctx, runtime, &expansion, value)?);
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
        let options = arguments.get(2..).ok_or(ObjectError::TypeError)?;
        values.extend_from_slice(options);
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
    let mut old = fresh_symbol(ctx, runtime)?;
    ncl_object::with_root(ctx, &mut old, |ctx, old| {
        let car = form(ctx, runtime, "CAR", &[*old])?;
        let cdr = form(ctx, runtime, "CDR", &[*old])?;
        let store = store_place(ctx, runtime, &expansion, cdr)?;
        let body = sequence(ctx, runtime, &[store, car])?;
        let old_binding = binding(ctx, runtime, *old, expansion.access_form)?;
        let body = wrap_let(ctx, runtime, "LET", &[old_binding], body)?;
        wrap_let(ctx, runtime, "LET*", &[], body)
    })
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
    let place_arguments = arguments.get(..place_count).ok_or(ObjectError::TypeError)?;
    for place_word in place_arguments {
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
            old_values
                .get(index + 1)
                .copied()
                .ok_or(ObjectError::TypeError)?
        } else if rotate {
            old_values.first().copied().ok_or(ObjectError::TypeError)?
        } else {
            arguments
                .get(place_count)
                .copied()
                .ok_or(ObjectError::TypeError)?
        };
        let expansion = expansions.get(index).ok_or(ObjectError::TypeError)?;
        stores.push(store_place(ctx, runtime, expansion, source)?);
    }
    stores.push(old_values.first().copied().ok_or(ObjectError::TypeError)?);
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
    let place_word = arguments.first().copied().ok_or(ObjectError::TypeError)?;
    let property = arguments.get(1).copied().ok_or(ObjectError::TypeError)?;
    let expansion = place(ctx, runtime, registry, place_word)?;
    with_expansion_roots(
        ctx,
        &expansion,
        |ctx, temporary_variables, value_forms, store_variables, store_form, access_form| {
            let rooted_expansion = SetfExpansion {
                temporary_variables: temporary_variables.to_vec(),
                value_forms: value_forms.to_vec(),
                store_variables: store_variables.to_vec(),
                store_form,
                access_form,
            };
            let mut access_form = access_form;
            ncl_object::with_root(ctx, &mut access_form, |ctx, access_form| {
                let mut property_list = fresh_symbol(ctx, runtime)?;
                ncl_object::with_root(ctx, &mut property_list, |ctx, property_list| {
                    let mut removed = form(ctx, runtime, "REMF", &[*property_list, property])?;
                    ncl_object::with_root(ctx, &mut removed, |ctx, removed| {
                        let mut store =
                            store_place(ctx, runtime, &rooted_expansion, *property_list)?;
                        ncl_object::with_root(ctx, &mut store, |ctx, store| {
                            let body = form(ctx, runtime, "IF", &[*removed, *store, Word::NIL])?;
                            let property_binding =
                                binding(ctx, runtime, *property_list, *access_form)?;
                            wrap_let(ctx, runtime, "LET", &[property_binding], body)
                        })
                    })
                })
            })
        },
    )
}

pub fn expand_get_setf_expansion(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    place_word: Word,
) -> Result<Vec<Word>, ObjectError> {
    let expansion = place(ctx, runtime, registry, place_word)?;
    let temporary_len = expansion.temporary_variables.len();
    let value_len = expansion.value_forms.len();
    let store_len = expansion.store_variables.len();
    let mut values = Vec::with_capacity(temporary_len + value_len + store_len + 2);
    values.extend_from_slice(&expansion.temporary_variables);
    values.extend_from_slice(&expansion.value_forms);
    values.extend_from_slice(&expansion.store_variables);
    values.push(expansion.store_form);
    values.push(expansion.access_form);
    ncl_object::with_roots(ctx, &values, |ctx, roots| {
        let temporary_values = roots
            .get(..temporary_len)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(|value| **value)
            .collect::<Vec<_>>();
        let value_values = roots
            .get(temporary_len..temporary_len + value_len)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(|value| **value)
            .collect::<Vec<_>>();
        let store_values = roots
            .get(temporary_len + value_len..temporary_len + value_len + store_len)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(|value| **value)
            .collect::<Vec<_>>();
        let store_form = roots
            .get(temporary_len + value_len + store_len)
            .map(|value| **value)
            .ok_or(ObjectError::TypeError)?;
        let access_form = roots
            .get(temporary_len + value_len + store_len + 1)
            .map(|value| **value)
            .ok_or(ObjectError::TypeError)?;
        ncl_object::with_roots(ctx, &[store_form, access_form], |ctx, fixed_roots| {
            let mut temporary_variables = list(ctx, runtime, &temporary_values)?;
            ncl_object::with_root(ctx, &mut temporary_variables, |ctx, temporary_variables| {
                let mut value_forms = list(ctx, runtime, &value_values)?;
                ncl_object::with_root(ctx, &mut value_forms, |ctx, value_forms| {
                    let mut store_variables = list(ctx, runtime, &store_values)?;
                    ncl_object::with_root(ctx, &mut store_variables, |_, store_variables| {
                        let store_form = fixed_roots
                            .first()
                            .map(|value| **value)
                            .ok_or(ObjectError::TypeError)?;
                        let access_form = fixed_roots
                            .get(1)
                            .map(|value| **value)
                            .ok_or(ObjectError::TypeError)?;
                        Ok(vec![
                            *temporary_variables,
                            *value_forms,
                            *store_variables,
                            store_form,
                            access_form,
                        ])
                    })
                })
            })
        })
    })
}

#[cfg(test)]
#[path = "setf_tests.rs"]
mod tests;

#![allow(missing_docs, clippy::missing_errors_doc)]
#![allow(clippy::trivially_copy_pass_by_ref, clippy::chunks_exact_to_as_chunks)]

use crate::{PlaceRegistry, SetfExpansion, elements, fresh_symbol, list, symbol};
use ncl_object::{
    Handle, HeapRootedWord, Local, ObjectError, ObjectRef, Runtime, Scope, ThreadContext, Word,
    classify_object,
};
#[path = "setf_defsetf.rs"]
mod defsetf;
use defsetf::defsetf_place;
#[path = "setf_ops.rs"]
mod ops;
use ops::RootedExpansion;
pub use ops::{
    expand_decf, expand_incf, expand_pop, expand_push, expand_remf, expand_rotatef, expand_shiftf,
};

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

type MacroEnvironment<'scope> = Vec<(Handle<'scope, Word>, Handle<'scope, Word>)>;

fn eval_quasiquote<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    datum: Handle<'scope, Word>,
    environment: &MacroEnvironment<'scope>,
    depth: u32,
) -> Result<Handle<'scope, Word>, ObjectError> {
    if let Some(inner) = defsetf::quasiquote_marker(scope, runtime, datum, "UNQUOTE")? {
        if depth == 1 {
            return defsetf::eval_macro_form(scope, runtime, inner, environment);
        }
        let inner = eval_quasiquote(scope, runtime, inner, environment, depth - 1)?;
        return defsetf::marker_form(scope, runtime, "UNQUOTE", inner);
    }
    if let Some(inner) = defsetf::quasiquote_marker(scope, runtime, datum, "QUASIQUOTE")? {
        let inner = eval_quasiquote(
            scope,
            runtime,
            inner,
            environment,
            depth.checked_add(1).ok_or(ObjectError::TypeError)?,
        )?;
        return defsetf::marker_form(scope, runtime, "QUASIQUOTE", inner);
    }
    if let Some(inner) = defsetf::quasiquote_marker(scope, runtime, datum, "UNQUOTE-SPLICING")? {
        if depth == 1 {
            return Err(ObjectError::TypeError);
        }
        let inner = eval_quasiquote(scope, runtime, inner, environment, depth - 1)?;
        return defsetf::marker_form(scope, runtime, "UNQUOTE-SPLICING", inner);
    }
    let word = scope.get(datum).as_word();
    if matches!(
        classify_object(scope.context(), word),
        ObjectRef::SimpleVector(_)
    ) {
        return defsetf::quasiquote_vector(scope, runtime, datum, environment, depth);
    }
    if !word.is_cons() {
        return Ok(datum);
    }
    let head_word = scope.car(datum)?.as_word();
    let head = scope.root(Local::from_word(head_word));
    let tail_word = scope.cdr(datum)?.as_word();
    let tail = scope.root(Local::from_word(tail_word));
    if depth == 1
        && let Some(inner) = defsetf::quasiquote_marker(scope, runtime, head, "UNQUOTE-SPLICING")?
    {
        let spliced = defsetf::eval_macro_form(scope, runtime, inner, environment)?;
        let items = scope.list_to_handle_vec(Local::from_word(scope.get(spliced).as_word()))?;
        let mut result = eval_quasiquote(scope, runtime, tail, environment, depth)?;
        for item in items.as_slice().iter().rev() {
            result = scope.make_cons(runtime, *item, result)?;
        }
        return Ok(result);
    }
    let head = eval_quasiquote(scope, runtime, head, environment, depth)?;
    let tail = eval_quasiquote(scope, runtime, tail, environment, depth)?;
    scope.make_cons(runtime, head, tail)
}

fn place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    place_word: Word,
) -> Result<SetfExpansion, ObjectError> {
    if !registry.belongs_to(runtime) {
        return Err(ObjectError::TypeError);
    }
    if matches!(classify_object(ctx, place_word), ObjectRef::Symbol(_)) {
        let mut place = place_word;
        return ncl_object::with_root(ctx, &mut place, |ctx, place| {
            let store = fresh_symbol(ctx, runtime)?;
            let store = HeapRootedWord::new(ctx, store)?;
            let store_form = form(ctx, runtime, "SETQ", &[*place, store.get()])?;
            Ok(SetfExpansion {
                temporary_variables: Vec::new(),
                value_forms: Vec::new(),
                store_variables: vec![store.get()],
                store_form,
                access_form: *place,
            })
        });
    }
    if !place_word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_heap_rooted_slice(ctx, &[place_word], |ctx, rooted_place| {
        let parts = elements(
            ctx,
            rooted_place.first().ok_or(ObjectError::TypeError)?.get(),
        )?;
        ncl_object::with_heap_rooted_slice(ctx, &parts, |ctx, parts| {
            let mut the = symbol(ctx, runtime, "THE")?;
            ncl_object::with_root(ctx, &mut the, |ctx, the| {
                let operator = parts.first().ok_or(ObjectError::TypeError)?.get();
                if operator == *the {
                    if parts.len() != 3 {
                        return Err(ObjectError::TypeError);
                    }
                    return place(
                        ctx,
                        runtime,
                        registry,
                        parts.get(2).ok_or(ObjectError::TypeError)?.get(),
                    );
                }
                let expansion = if let Some(expander) = registry.get(ctx, operator)? {
                    let arguments = parts
                        .iter()
                        .skip(1)
                        .map(HeapRootedWord::get)
                        .collect::<Vec<_>>();
                    expander(ctx, runtime, &arguments)?
                } else {
                    let arguments = parts
                        .iter()
                        .skip(1)
                        .map(HeapRootedWord::get)
                        .collect::<Vec<_>>();
                    if let Some(expansion) = defsetf_place(ctx, runtime, operator, &arguments)? {
                        expansion
                    } else {
                        let operator = parts.first().ok_or(ObjectError::TypeError)?.get();
                        let arguments = parts
                            .iter()
                            .skip(1)
                            .map(HeapRootedWord::get)
                            .collect::<Vec<_>>();
                        crate::setf_places::standard_cons_place(ctx, runtime, operator, &arguments)?
                            .ok_or(ObjectError::UndefinedFunction)?
                    }
                };
                validate_expansion(&expansion)?;
                Ok(expansion)
            })
        })
    })
}

pub(super) fn place_value(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    place: Word,
    value: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_heap_rooted_slice(ctx, &[place, value], |ctx, roots| {
        let mut current =
            HeapRootedWord::new(ctx, roots.first().ok_or(ObjectError::TypeError)?.get())?;
        let mut result =
            HeapRootedWord::new(ctx, roots.get(1).ok_or(ObjectError::TypeError)?.get())?;
        let mut types = Vec::<HeapRootedWord>::new();
        loop {
            let form_word = current.get();
            if !form_word.is_cons() {
                break;
            }
            let parts = elements(ctx, form_word)?;
            let found = ncl_object::with_heap_rooted_slice(ctx, &parts, |ctx, parts| {
                let mut the = symbol(ctx, runtime, "THE")?;
                ncl_object::with_root(ctx, &mut the, |ctx, the| {
                    if parts.first().ok_or(ObjectError::TypeError)?.get() != *the {
                        return Ok(false);
                    }
                    if parts.len() != 3 {
                        return Err(ObjectError::TypeError);
                    }
                    let type_form = HeapRootedWord::new(
                        ctx,
                        parts.get(1).ok_or(ObjectError::TypeError)?.get(),
                    )?;
                    current = HeapRootedWord::new(
                        ctx,
                        parts.get(2).ok_or(ObjectError::TypeError)?.get(),
                    )?;
                    types.push(type_form);
                    Ok(true)
                })
            })?;
            if !found {
                break;
            }
        }
        for type_form in types {
            let wrapped = form(ctx, runtime, "THE", &[type_form.get(), result.get()])?;
            result = HeapRootedWord::new(ctx, wrapped)?;
        }
        Ok(result.get())
    })
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
    let mut value = value;
    ncl_object::with_root(ctx, &mut value, |ctx, value| {
        with_expansion_roots(
            ctx,
            expansion,
            |ctx, temporary, values, stores, store_form, _| {
                let mut bindings = Vec::<HeapRootedWord>::with_capacity(temporary.len());
                for (variable, value_form) in temporary.iter().zip(values) {
                    let binding = binding(ctx, runtime, variable.get(), value_form.get())?;
                    bindings.push(HeapRootedWord::new(ctx, binding)?);
                }
                let body = store_place_prebound_roots(ctx, runtime, stores, store_form, *value)?;
                if bindings.is_empty() {
                    Ok(body)
                } else {
                    let bindings = bindings.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
                    wrap_let(ctx, runtime, "LET*", &bindings, body)
                }
            },
        )
    })
}

pub(super) fn store_place_prebound(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: &SetfExpansion,
    value: Word,
) -> Result<Word, ObjectError> {
    let mut value = value;
    ncl_object::with_root(ctx, &mut value, |ctx, value| {
        with_expansion_roots(ctx, expansion, |ctx, _, _, stores, store_form, _| {
            store_place_prebound_roots(ctx, runtime, stores, store_form, *value)
        })
    })
}

fn store_place_prebound_roots(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    stores: &[HeapRootedWord],
    store_form: &HeapRootedWord,
    value: Word,
) -> Result<Word, ObjectError> {
    let store_variable = stores.first().ok_or(ObjectError::TypeError)?.get();
    let store_binding = binding(ctx, runtime, store_variable, value)?;
    wrap_let(ctx, runtime, "LET", &[store_binding], store_form.get())
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
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        if parallel {
            let mut evaluation_bindings = Vec::<HeapRootedWord>::new();
            let mut stores = Vec::new();
            for pair in arguments.chunks_exact(2) {
                let place_word = pair.first().ok_or(ObjectError::TypeError)?;
                let value = pair.get(1).ok_or(ObjectError::TypeError)?;
                let expansion = place(ctx, runtime, registry, place_word.get())?;
                let expansion = RootedExpansion::new(ctx, expansion)?;
                for (variable, value_form) in expansion
                    .temporary_variables
                    .iter()
                    .zip(&expansion.value_forms)
                {
                    let binding = binding(ctx, runtime, variable.get(), value_form.get())?;
                    evaluation_bindings.push(HeapRootedWord::new(ctx, binding)?);
                }
                let value_variable = fresh_symbol(ctx, runtime)?;
                let value_variable = HeapRootedWord::new(ctx, value_variable)?;
                let value = place_value(ctx, runtime, place_word.get(), value.get())?;
                let value = HeapRootedWord::new(ctx, value)?;
                let binding = binding(ctx, runtime, value_variable.get(), value.get())?;
                evaluation_bindings.push(HeapRootedWord::new(ctx, binding)?);
                stores.push((expansion, value_variable));
            }
            let mut store_forms = Vec::new();
            for (expansion, value_variable) in stores {
                let store =
                    store_place_prebound(ctx, runtime, &expansion.current(), value_variable.get())?;
                store_forms.push(HeapRootedWord::new(ctx, store)?);
            }
            let mut forms = store_forms
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            forms.push(Word::NIL);
            let body = sequence(ctx, runtime, &forms)?;
            let body = HeapRootedWord::new(ctx, body)?;
            if evaluation_bindings.is_empty() {
                Ok(body.get())
            } else {
                let bindings = evaluation_bindings
                    .iter()
                    .map(HeapRootedWord::get)
                    .collect::<Vec<_>>();
                wrap_let(ctx, runtime, "LET*", &bindings, body.get())
            }
        } else {
            let mut forms = Vec::<HeapRootedWord>::new();
            for pair in arguments.chunks_exact(2) {
                let place_word = pair.first().ok_or(ObjectError::TypeError)?;
                let value = pair.get(1).ok_or(ObjectError::TypeError)?;
                let expansion = place(ctx, runtime, registry, place_word.get())?;
                let expansion = RootedExpansion::new(ctx, expansion)?;
                let value = place_value(ctx, runtime, place_word.get(), value.get())?;
                let value = HeapRootedWord::new(ctx, value)?;
                let store = store_place(ctx, runtime, &expansion.current(), value.get())?;
                forms.push(HeapRootedWord::new(ctx, store)?);
            }
            let forms = forms.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
            sequence(ctx, runtime, &forms)
        }
    })
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

pub fn expand_get_setf_expansion(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    place_word: Word,
) -> Result<SetfExpansion, ObjectError> {
    let mut place_word = place_word;
    ncl_object::with_root(ctx, &mut place_word, |ctx, place_word| {
        let expansion = place(ctx, runtime, registry, *place_word)?;
        with_expansion_roots(
            ctx,
            &expansion,
            |ctx, temporary, values, stores, store_form, access_form| {
                let store = stores.first().ok_or(ObjectError::TypeError)?.get();
                let typed_store = place_value(ctx, runtime, *place_word, store)?;
                let store_form = if typed_store == store {
                    store_form.get()
                } else {
                    let binding = binding(ctx, runtime, store, typed_store)?;
                    wrap_let(ctx, runtime, "LET", &[binding], store_form.get())?
                };
                Ok(SetfExpansion {
                    temporary_variables: temporary.iter().map(HeapRootedWord::get).collect(),
                    value_forms: values.iter().map(HeapRootedWord::get).collect(),
                    store_variables: stores.iter().map(HeapRootedWord::get).collect(),
                    store_form,
                    access_form: access_form.get(),
                })
            },
        )
    })
}

#[cfg(test)]
#[path = "setf_tests.rs"]
mod tests;

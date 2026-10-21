use super::{SetfExpansion, binding, form, place, sequence, store_place, wrap_let};
use crate::{PlaceRegistry, fresh_symbol};
use ncl_object::{HeapRootedWord, ObjectError, Runtime, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub(super) struct RootedExpansion {
    pub(super) temporary_variables: Vec<HeapRootedWord>,
    pub(super) value_forms: Vec<HeapRootedWord>,
    store_variables: Vec<HeapRootedWord>,
    store_form: HeapRootedWord,
    pub(super) access_form: HeapRootedWord,
}

impl RootedExpansion {
    pub(super) fn new(ctx: &mut ThreadContext, expansion: SetfExpansion) -> Result<Self> {
        if expansion.temporary_variables.len() != expansion.value_forms.len()
            || expansion.store_variables.len() != 1
        {
            return Err(ObjectError::TypeError);
        }
        let temporary_len = expansion.temporary_variables.len();
        let value_len = expansion.value_forms.len();
        let mut words = expansion.temporary_variables;
        words.extend(expansion.value_forms);
        words.extend(expansion.store_variables);
        words.push(expansion.store_form);
        words.push(expansion.access_form);
        let mut roots = ncl_object::with_roots(ctx, &words, |ctx, roots| {
            roots
                .iter()
                .map(|word| HeapRootedWord::new(ctx, **word))
                .collect::<Result<Vec<_>>>()
        })?;
        let access_form = roots.pop().ok_or(ObjectError::TypeError)?;
        let store_form = roots.pop().ok_or(ObjectError::TypeError)?;
        let store_variables = roots.split_off(temporary_len + value_len);
        let value_forms = roots.split_off(temporary_len);
        Ok(Self {
            temporary_variables: roots,
            value_forms,
            store_variables,
            store_form,
            access_form,
        })
    }

    pub(super) fn current(&self) -> SetfExpansion {
        SetfExpansion {
            temporary_variables: self
                .temporary_variables
                .iter()
                .map(HeapRootedWord::get)
                .collect(),
            value_forms: self.value_forms.iter().map(HeapRootedWord::get).collect(),
            store_variables: self
                .store_variables
                .iter()
                .map(HeapRootedWord::get)
                .collect(),
            store_form: self.store_form.get(),
            access_form: self.access_form.get(),
        }
    }
}

fn modify(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
    operator: &str,
) -> Result {
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let place_word = arguments.first().ok_or(ObjectError::TypeError)?.get();
        let delta = arguments
            .get(1)
            .map_or(Word::fixnum(1), HeapRootedWord::get);
        let expansion = place(ctx, runtime, registry, place_word)?;
        let expansion = RootedExpansion::new(ctx, expansion)?;
        let access = expansion.access_form.get();
        let arithmetic = form(ctx, runtime, operator, &[access, delta])?;
        store_place(ctx, runtime, &expansion.current(), arithmetic)
    })
}

pub fn expand_incf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result {
    modify(ctx, runtime, registry, arguments, "+")
}

pub fn expand_decf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result {
    modify(ctx, runtime, registry, arguments, "-")
}

pub fn expand_push(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
    new_only: bool,
) -> Result {
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let item = arguments.first().ok_or(ObjectError::TypeError)?.get();
        let place_word = arguments.get(1).ok_or(ObjectError::TypeError)?.get();
        let expansion = place(ctx, runtime, registry, place_word)?;
        let expansion = RootedExpansion::new(ctx, expansion)?;
        let operator = if new_only { "ADJOIN" } else { "CONS" };
        let mut values = vec![item, expansion.access_form.get()];
        if new_only {
            values.extend(arguments.iter().skip(2).map(HeapRootedWord::get));
        }
        let value = form(ctx, runtime, operator, &values)?;
        store_place(ctx, runtime, &expansion.current(), value)
    })
}

pub fn expand_pop(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result {
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let place_word = arguments.first().ok_or(ObjectError::TypeError)?.get();
        let expansion = place(ctx, runtime, registry, place_word)?;
        let expansion = RootedExpansion::new(ctx, expansion)?;
        let old = fresh_symbol(ctx, runtime)?;
        let old = HeapRootedWord::new(ctx, old)?;
        let car = form(ctx, runtime, "CAR", &[old.get()])?;
        let car = HeapRootedWord::new(ctx, car)?;
        let cdr = form(ctx, runtime, "CDR", &[old.get()])?;
        let cdr = HeapRootedWord::new(ctx, cdr)?;
        let store = store_place(ctx, runtime, &expansion.current(), cdr.get())?;
        let store = HeapRootedWord::new(ctx, store)?;
        let body = sequence(ctx, runtime, &[store.get(), car.get()])?;
        let body = HeapRootedWord::new(ctx, body)?;
        let old_binding = binding(ctx, runtime, old.get(), expansion.access_form.get())?;
        let body = wrap_let(ctx, runtime, "LET", &[old_binding], body.get())?;
        wrap_let(ctx, runtime, "LET*", &[], body)
    })
}

pub fn expand_shiftf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result {
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
) -> Result {
    if arguments.is_empty() {
        return Ok(Word::NIL);
    }
    rotate_like(ctx, runtime, registry, arguments, true)
}

fn rotate_like(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
    rotate: bool,
) -> Result {
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let place_count = if rotate {
            arguments.len()
        } else {
            arguments.len() - 1
        };
        let mut expansions = Vec::new();
        let mut bindings = Vec::<HeapRootedWord>::new();
        let mut old_values = Vec::<HeapRootedWord>::new();
        for place_word in arguments.iter().take(place_count) {
            let expansion = place(ctx, runtime, registry, place_word.get())?;
            let expansion = RootedExpansion::new(ctx, expansion)?;
            let old = fresh_symbol(ctx, runtime)?;
            let old = HeapRootedWord::new(ctx, old)?;
            for (variable, value_form) in expansion
                .temporary_variables
                .iter()
                .zip(&expansion.value_forms)
            {
                let binding = binding(ctx, runtime, variable.get(), value_form.get())?;
                bindings.push(HeapRootedWord::new(ctx, binding)?);
            }
            let old_binding = binding(ctx, runtime, old.get(), expansion.access_form.get())?;
            bindings.push(HeapRootedWord::new(ctx, old_binding)?);
            expansions.push(expansion);
            old_values.push(old);
        }
        let mut stores = Vec::<HeapRootedWord>::new();
        for index in 0..place_count {
            let source = if index + 1 < place_count {
                old_values
                    .get(index + 1)
                    .ok_or(ObjectError::TypeError)?
                    .get()
            } else if rotate {
                old_values.first().ok_or(ObjectError::TypeError)?.get()
            } else {
                arguments
                    .get(place_count)
                    .ok_or(ObjectError::TypeError)?
                    .get()
            };
            let expansion = expansions.get(index).ok_or(ObjectError::TypeError)?;
            let store = store_place(ctx, runtime, &expansion.current(), source)?;
            stores.push(HeapRootedWord::new(ctx, store)?);
        }
        let mut store_forms = stores.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
        store_forms.push(if rotate {
            Word::NIL
        } else {
            old_values.first().ok_or(ObjectError::TypeError)?.get()
        });
        let body = sequence(ctx, runtime, &store_forms)?;
        let binding_forms = bindings.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
        wrap_let(ctx, runtime, "LET*", &binding_forms, body)
    })
}

pub fn expand_remf(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result {
    if arguments.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let place_word = arguments.first().ok_or(ObjectError::TypeError)?.get();
        let property = arguments.get(1).ok_or(ObjectError::TypeError)?.get();
        let expansion = place(ctx, runtime, registry, place_word)?;
        let expansion = RootedExpansion::new(ctx, expansion)?;
        let property_list = fresh_symbol(ctx, runtime)?;
        let property_list = HeapRootedWord::new(ctx, property_list)?;
        let removed = form(ctx, runtime, "REMF", &[property_list.get(), property])?;
        let removed = HeapRootedWord::new(ctx, removed)?;
        let store = store_place(ctx, runtime, &expansion.current(), property_list.get())?;
        let store = HeapRootedWord::new(ctx, store)?;
        let body = form(ctx, runtime, "IF", &[removed.get(), store.get(), Word::NIL])?;
        let body = HeapRootedWord::new(ctx, body)?;
        let property_binding = binding(
            ctx,
            runtime,
            property_list.get(),
            expansion.access_form.get(),
        )?;
        wrap_let(ctx, runtime, "LET", &[property_binding], body.get())
    })
}

use super::{
    SetfExpansion, binding, form, place, place_value, sequence, store_place_prebound, wrap_let,
};
use crate::{PlaceRegistry, fresh_symbol};
use ncl_object::{
    HeapRootedWord, ObjectError, ObjectRef, Runtime, ThreadContext, Word, classify_object,
    string_length, string_ref, symbol_name, symbol_package,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

#[path = "setf_ops_remf.rs"]
mod remf;
pub use remf::expand_remf;

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

fn add_place_bindings(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    expansion: &RootedExpansion,
    bindings: &mut Vec<HeapRootedWord>,
) -> Result<()> {
    for (variable, value_form) in expansion
        .temporary_variables
        .iter()
        .zip(&expansion.value_forms)
    {
        let binding = binding(ctx, runtime, variable.get(), value_form.get())?;
        bindings.push(HeapRootedWord::new(ctx, binding)?);
    }
    Ok(())
}

fn add_binding(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    bindings: &mut Vec<HeapRootedWord>,
    variable: Word,
    value: Word,
) -> Result<()> {
    let binding = binding(ctx, runtime, variable, value)?;
    bindings.push(HeapRootedWord::new(ctx, binding)?);
    Ok(())
}

fn wrap_bindings(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    bindings: &[HeapRootedWord],
    body: Word,
) -> Result {
    if bindings.is_empty() {
        return Ok(body);
    }
    let bindings = bindings.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
    wrap_let(ctx, runtime, "LET*", &bindings, body)
}

fn pushnew_option_pairs(
    ctx: &ThreadContext,
    runtime: &Runtime,
    args: &[HeapRootedWord],
) -> Result<Vec<(usize, usize)>> {
    if !args.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    let keyword_package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::TypeError)?;
    let mut test_seen = false;
    let mut test_not_seen = false;
    let mut pairs = Vec::with_capacity(args.len() / 2);
    for index in (0..args.len()).step_by(2) {
        let keyword = args.get(index).ok_or(ObjectError::TypeError)?.get();
        let ObjectRef::Symbol(symbol) = classify_object(ctx, keyword) else {
            return Err(ObjectError::TypeError);
        };
        if symbol_package(ctx, symbol)? != keyword_package {
            return Err(ObjectError::TypeError);
        }
        let name_word = symbol_name(ctx, symbol)?;
        let name = (0..string_length(ctx, name_word)?)
            .map(|character| string_ref(ctx, name_word, character))
            .collect::<std::result::Result<String, _>>()?;
        if name == "TEST" {
            test_seen = true;
        } else if name == "TEST-NOT" {
            test_not_seen = true;
        } else if name != "KEY" {
            return Err(ObjectError::TypeError);
        }
        if test_seen && test_not_seen {
            return Err(ObjectError::TypeError);
        }
        pairs.push((index, index.checked_add(1).ok_or(ObjectError::TypeError)?));
    }
    Ok(pairs)
}

fn modify(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
    operator: &str,
) -> Result {
    if !(1..=2).contains(&arguments.len()) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let place_word = arguments.first().ok_or(ObjectError::TypeError)?;
        let expansion = place(ctx, runtime, registry, place_word.get())?;
        let expansion = RootedExpansion::new(ctx, expansion)?;
        let mut bindings = Vec::new();
        add_place_bindings(ctx, runtime, &expansion, &mut bindings)?;

        let delta = fresh_symbol(ctx, runtime)?;
        let delta = HeapRootedWord::new(ctx, delta)?;
        let delta_form = arguments
            .get(1)
            .map_or(Word::fixnum(1), HeapRootedWord::get);
        add_binding(ctx, runtime, &mut bindings, delta.get(), delta_form)?;

        let old = fresh_symbol(ctx, runtime)?;
        let old = HeapRootedWord::new(ctx, old)?;
        add_binding(
            ctx,
            runtime,
            &mut bindings,
            old.get(),
            expansion.access_form.get(),
        )?;
        let arithmetic = form(ctx, runtime, operator, &[old.get(), delta.get()])?;
        let value = place_value(ctx, runtime, place_word.get(), arithmetic)?;
        let value = HeapRootedWord::new(ctx, value)?;
        let store = store_place_prebound(ctx, runtime, &expansion.current(), value.get())?;
        wrap_bindings(ctx, runtime, &bindings, store)
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
    if (!new_only && arguments.len() != 2) || (new_only && arguments.len() < 2) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let item = arguments.first().ok_or(ObjectError::TypeError)?;
        let place_word = arguments.get(1).ok_or(ObjectError::TypeError)?;
        let option_arguments = arguments.get(2..).ok_or(ObjectError::TypeError)?;
        let option_pairs = if new_only {
            pushnew_option_pairs(ctx, runtime, option_arguments)?
        } else {
            Vec::new()
        };
        let expansion = place(ctx, runtime, registry, place_word.get())?;
        let expansion = RootedExpansion::new(ctx, expansion)?;
        let mut bindings = Vec::new();
        let item_variable = fresh_symbol(ctx, runtime)?;
        let item_variable = HeapRootedWord::new(ctx, item_variable)?;
        add_binding(ctx, runtime, &mut bindings, item_variable.get(), item.get())?;
        add_place_bindings(ctx, runtime, &expansion, &mut bindings)?;

        let operator = if new_only { "ADJOIN" } else { "CONS" };
        let mut values = vec![item_variable.get(), expansion.access_form.get()];
        for (keyword_index, value_index) in option_pairs {
            let keyword = option_arguments
                .get(keyword_index)
                .ok_or(ObjectError::TypeError)?
                .get();
            let value_form = option_arguments
                .get(value_index)
                .ok_or(ObjectError::TypeError)?
                .get();
            let value_variable = fresh_symbol(ctx, runtime)?;
            let value_variable = HeapRootedWord::new(ctx, value_variable)?;
            add_binding(
                ctx,
                runtime,
                &mut bindings,
                value_variable.get(),
                value_form,
            )?;
            values.push(keyword);
            values.push(value_variable.get());
        }
        let value = form(ctx, runtime, operator, &values)?;
        let value = place_value(ctx, runtime, place_word.get(), value)?;
        let value = HeapRootedWord::new(ctx, value)?;
        let store = store_place_prebound(ctx, runtime, &expansion.current(), value.get())?;
        wrap_bindings(ctx, runtime, &bindings, store)
    })
}

pub fn expand_pop(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    registry: &PlaceRegistry,
    arguments: &[Word],
) -> Result {
    if arguments.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::with_heap_rooted_slice(ctx, arguments, |ctx, arguments| {
        let place_word = arguments.first().ok_or(ObjectError::TypeError)?;
        let expansion = place(ctx, runtime, registry, place_word.get())?;
        let expansion = RootedExpansion::new(ctx, expansion)?;
        let mut bindings = Vec::new();
        add_place_bindings(ctx, runtime, &expansion, &mut bindings)?;
        let old = fresh_symbol(ctx, runtime)?;
        let old = HeapRootedWord::new(ctx, old)?;
        add_binding(
            ctx,
            runtime,
            &mut bindings,
            old.get(),
            expansion.access_form.get(),
        )?;
        let car = form(ctx, runtime, "CAR", &[old.get()])?;
        let car = HeapRootedWord::new(ctx, car)?;
        let car_value = fresh_symbol(ctx, runtime)?;
        let car_value = HeapRootedWord::new(ctx, car_value)?;
        add_binding(ctx, runtime, &mut bindings, car_value.get(), car.get())?;
        let cdr = form(ctx, runtime, "CDR", &[old.get()])?;
        let cdr = HeapRootedWord::new(ctx, cdr)?;
        let value = place_value(ctx, runtime, place_word.get(), cdr.get())?;
        let value = HeapRootedWord::new(ctx, value)?;
        let store = store_place_prebound(ctx, runtime, &expansion.current(), value.get())?;
        let store = HeapRootedWord::new(ctx, store)?;
        let body = sequence(ctx, runtime, &[store.get(), car_value.get()])?;
        wrap_bindings(ctx, runtime, &bindings, body)
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
            add_place_bindings(ctx, runtime, &expansion, &mut bindings)?;
            expansions.push(expansion);
        }

        let shift_value = if rotate {
            None
        } else {
            let variable = fresh_symbol(ctx, runtime)?;
            let variable = HeapRootedWord::new(ctx, variable)?;
            let value = arguments
                .get(place_count)
                .ok_or(ObjectError::TypeError)?
                .get();
            add_binding(ctx, runtime, &mut bindings, variable.get(), value)?;
            Some(variable)
        };

        for expansion in &expansions {
            let old = fresh_symbol(ctx, runtime)?;
            let old = HeapRootedWord::new(ctx, old)?;
            add_binding(
                ctx,
                runtime,
                &mut bindings,
                old.get(),
                expansion.access_form.get(),
            )?;
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
                shift_value.as_ref().ok_or(ObjectError::TypeError)?.get()
            };
            let expansion = expansions.get(index).ok_or(ObjectError::TypeError)?;
            let target = arguments.get(index).ok_or(ObjectError::TypeError)?.get();
            let source = place_value(ctx, runtime, target, source)?;
            let source = HeapRootedWord::new(ctx, source)?;
            let store = store_place_prebound(ctx, runtime, &expansion.current(), source.get())?;
            stores.push(HeapRootedWord::new(ctx, store)?);
        }
        let mut store_forms = stores.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
        store_forms.push(if rotate {
            Word::NIL
        } else {
            old_values.first().ok_or(ObjectError::TypeError)?.get()
        });
        let body = sequence(ctx, runtime, &store_forms)?;
        wrap_bindings(ctx, runtime, &bindings, body)
    })
}

use super::*;
use crate::form::elements;
use crate::place::PlaceRegistry;
use crate::setf::{
    expand_decf, expand_get_setf_expansion, expand_incf, expand_pop, expand_psetf, expand_push,
    expand_remf, expand_rotatef, expand_setf, expand_shiftf,
};
use ncl_object::{HeapRootedWord, Runtime, ThreadContext};

fn test_operation_failure<T>(error: impl std::fmt::Debug) -> T {
    std::panic::panic_any(format!("test operation failed: {error:?}"))
}

fn registry_with_builtins(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    // The accessor builtins referenced by these expansions (CAR, RPLACA,
    // SET, ...) live in other crates and are not needed to build the
    // expansion form itself; only the place-expander registration (this
    // crate's `register`) and symbol interning are required here.
    ctx.register(runtime)?;
    crate::register(runtime)?;
    Ok(())
}

fn compound_accessor_names() -> Vec<String> {
    (1..=4)
        .flat_map(|depth| {
            (0..(1 << depth)).map(move |bits| {
                let path = (0..depth)
                    .map(|index| if bits & (1 << index) == 0 { 'A' } else { 'D' })
                    .collect::<String>();
                format!("C{path}R")
            })
        })
        .collect()
}

fn make_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: &str,
    variable: Word,
) -> Result<HeapRootedWord, ObjectError> {
    let operator = symbol(ctx, runtime, operator)?;
    let operator = HeapRootedWord::new(ctx, operator)?;
    let place = list(ctx, runtime, &[operator.get(), variable])?;
    HeapRootedWord::new(ctx, place)
}

fn assert_expansion(
    ctx: &mut ThreadContext,
    result: Result<Word, ObjectError>,
    place_name: &str,
    operation: &str,
) -> Result<(), ObjectError> {
    let succeeded = result.is_ok();
    assert!(
        succeeded,
        "{operation} expansion failed for {place_name}: {:?}",
        result.as_ref().err()
    );
    let Some(mut expansion) = result.ok() else {
        return Ok(());
    };
    let root = ncl_object::push_root(ctx, &mut expansion);
    let nonempty = !elements(ctx, expansion)?.is_empty();
    let popped = ncl_object::pop_root(ctx, root);
    assert!(nonempty, "{operation} expansion was empty for {place_name}");
    assert!(
        popped,
        "{operation} expansion root was lost for {place_name}"
    );
    Ok(())
}

fn contains_nested_the(
    ctx: &mut ThreadContext,
    form: Word,
    the: Word,
    outermost_type: Word,
    nested_type: Word,
) -> Result<bool, ObjectError> {
    if !form.is_cons() {
        return Ok(false);
    }
    let parts = elements(ctx, form)?;
    if parts.len() == 3 && parts[0] == the && parts[1] == outermost_type && parts[2].is_cons() {
        let inner = elements(ctx, parts[2])?;
        if inner.len() == 3 && inner[0] == the && inner[1] == nested_type {
            return Ok(true);
        }
    }
    for part in parts {
        if contains_nested_the(ctx, part, the, outermost_type, nested_type)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn contains_operator(
    ctx: &mut ThreadContext,
    form: Word,
    operator: Word,
) -> Result<bool, ObjectError> {
    if !form.is_cons() {
        return Ok(false);
    }
    let parts = elements(ctx, form)?;
    if parts.first().copied() == Some(operator) {
        return Ok(true);
    }
    for part in parts {
        if contains_operator(ctx, part, operator)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn contains_form(ctx: &mut ThreadContext, form: Word, expected: Word) -> Result<bool, ObjectError> {
    if form == expected {
        return Ok(true);
    }
    if !form.is_cons() {
        return Ok(false);
    }
    for part in elements(ctx, form)? {
        if contains_form(ctx, part, expected)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn place_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    operator: &str,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    let operator = symbol(ctx, runtime, operator)?;
    let mut parts = Vec::with_capacity(arguments.len() + 1);
    parts.push(operator);
    parts.extend_from_slice(arguments);
    list(ctx, runtime, &parts)
}

#[path = "setf_places_tests/standard_places.rs"]
mod standard_places;

#[path = "setf_places_tests/modifying_operators.rs"]
mod modifying_operators;

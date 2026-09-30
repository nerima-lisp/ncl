//! Backquote (quasiquote) expansion, per CLHS 2.4.6.
#![allow(clippy::redundant_pub_crate)]

use crate::{elements, list, symbol};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word, car, cdr,
    classify_object, simple_vector_length, simple_vector_ref,
};

type Result<T = usize> = std::result::Result<T, ObjectError>;

/// Every intermediate value lives in `held`, a vector rooted afresh before
/// each allocation (mirroring `iteration.rs`'s `held_form`), so a form built
/// deep in the recursion cannot be invalidated by garbage collection
/// triggered later in the same expansion.
fn held_get(held: &[Word], index: usize) -> std::result::Result<Word, ObjectError> {
    held.get(index).copied().ok_or(ObjectError::TypeError)
}

fn held_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: &str,
) -> Result {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = symbol(ctx, runtime, name)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

fn held_call(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: &str,
    indexes: &[usize],
) -> Result {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let args = indexes
            .iter()
            .map(|index| {
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::TypeError)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let operator = symbol(ctx, runtime, name)?;
        let mut values = Vec::with_capacity(args.len() + 1);
        values.push(operator);
        values.extend(args);
        let value = list(ctx, runtime, &values)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

/// Whether `held[index]` is `(marker arg)`, a proper two-element list headed
/// by the symbol at `held[marker]`. Reads only; performs no allocation.
fn heading(
    ctx: &ThreadContext,
    held: &mut Vec<Word>,
    index: usize,
    marker: usize,
) -> Result<Option<usize>> {
    let word = held_get(held, index)?;
    if !word.is_cons() {
        return Ok(None);
    }
    let car_word = car(ctx, word)?;
    if car_word != held_get(held, marker)? {
        return Ok(None);
    }
    let rest = cdr(ctx, word)?;
    if !rest.is_cons() {
        return Ok(None);
    }
    let arg = car(ctx, rest)?;
    if cdr(ctx, rest)? != Word::NIL {
        return Ok(None);
    }
    held.push(arg);
    Ok(Some(held.len() - 1))
}

/// The three reader-produced backquote marker symbols, resolved once.
struct Markers {
    unquote: usize,
    splice: usize,
    quasiquote: usize,
}

/// Build code that reconstructs `(marker arg)` as data, for a marker form
/// nested inside a deeper backquote than the one currently expanding.
fn reconstruct(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    marker: usize,
    arg: usize,
) -> Result {
    let quoted_marker = held_call(ctx, runtime, held, "QUOTE", &[marker])?;
    held_call(ctx, runtime, held, "LIST", &[quoted_marker, arg])
}

/// Expand one backquoted datum at nesting `depth` (starting at 1) into code
/// that, when evaluated, builds the substituted data structure.
fn qq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    index: usize,
    depth: u32,
    markers: &Markers,
) -> Result {
    if let Some(inner) = heading(ctx, held, index, markers.unquote)? {
        if depth == 1 {
            return Ok(inner);
        }
        let reconstructed = qq(ctx, runtime, held, inner, depth - 1, markers)?;
        return reconstruct(ctx, runtime, held, markers.unquote, reconstructed);
    }
    if let Some(inner) = heading(ctx, held, index, markers.splice)? {
        if depth == 1 {
            // `,@` (or `,.`) is only meaningful as a list or vector element;
            // CLHS 2.4.6 treats a bare splice at this depth as an error.
            return Err(ObjectError::TypeError);
        }
        let reconstructed = qq(ctx, runtime, held, inner, depth - 1, markers)?;
        return reconstruct(ctx, runtime, held, markers.splice, reconstructed);
    }
    if let Some(inner) = heading(ctx, held, index, markers.quasiquote)? {
        let reconstructed = qq(ctx, runtime, held, inner, depth + 1, markers)?;
        return reconstruct(ctx, runtime, held, markers.quasiquote, reconstructed);
    }
    let word = held_get(held, index)?;
    match classify_object(ctx, word) {
        ObjectRef::Cons(_) => qq_list(ctx, runtime, held, index, depth, markers),
        ObjectRef::SimpleVector(_) => qq_vector(ctx, runtime, held, index, depth, markers),
        // check-added-lines: allow(wildcard) every other datum is self-evaluating and just needs quoting.
        _ => held_call(ctx, runtime, held, "QUOTE", &[index]),
    }
}

/// Expand a cons, handling `,@`/`,.` splicing at any position and the
/// dotted-tail case `(a . ,b)`, which the reader hands back as `(a unquote
/// b)` and this walk naturally resolves through the `unquote` branch of
/// [`qq`] applied to the tail.
fn qq_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    index: usize,
    depth: u32,
    markers: &Markers,
) -> Result {
    let word = held_get(held, index)?;
    let car_word = car(ctx, word)?;
    let tail = cdr(ctx, word)?;
    held.push(car_word);
    let car_index = held.len() - 1;
    held.push(tail);
    let tail_index = held.len() - 1;
    if let Some(spliced) = heading(ctx, held, car_index, markers.splice)? {
        let tail_code = qq(ctx, runtime, held, tail_index, depth, markers)?;
        if depth == 1 {
            return held_call(ctx, runtime, held, "APPEND", &[spliced, tail_code]);
        }
        let inner_code = qq(ctx, runtime, held, spliced, depth - 1, markers)?;
        let reconstructed_car = reconstruct(ctx, runtime, held, markers.splice, inner_code)?;
        return held_call(ctx, runtime, held, "CONS", &[reconstructed_car, tail_code]);
    }
    let car_code = qq(ctx, runtime, held, car_index, depth, markers)?;
    let tail_code = qq(ctx, runtime, held, tail_index, depth, markers)?;
    held_call(ctx, runtime, held, "CONS", &[car_code, tail_code])
}

/// Expand `#(...)` by reusing the list algorithm over its elements, then
/// applying `vector` to the reconstructed list so `,@` works inside vectors.
fn qq_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    index: usize,
    depth: u32,
    markers: &Markers,
) -> Result {
    let word = held_get(held, index)?;
    let length = simple_vector_length(ctx, word)?;
    let mut elements_vec = Vec::with_capacity(length);
    for element_index in 0..length {
        elements_vec.push(simple_vector_ref(ctx, word, element_index)?);
    }
    let (as_list, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let as_list = list(ctx, runtime, &elements_vec)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((as_list, refreshed))
    })?;
    *held = refreshed;
    held.push(as_list);
    let list_index = held.len() - 1;
    let list_code = qq(ctx, runtime, held, list_index, depth, markers)?;
    let vector_symbol = held_symbol(ctx, runtime, held, "VECTOR")?;
    let vector_function = held_call(ctx, runtime, held, "FUNCTION", &[vector_symbol])?;
    held_call(ctx, runtime, held, "APPLY", &[vector_function, list_code])
}

pub fn expand_quasiquote_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> std::result::Result<Word, ObjectError> {
    let form = args.get(0).ok_or(ObjectError::TypeError)?;
    let parts = elements(ctx, form)?;
    // check-added-lines: allow(index) slice-pattern destructuring, not indexing.
    let [_quasiquote, datum] = parts.as_slice() else {
        return Err(ObjectError::TypeError);
    };
    let mut held = vec![*datum];
    let unquote = held_symbol(ctx, runtime, &mut held, "UNQUOTE")?;
    let splice = held_symbol(ctx, runtime, &mut held, "UNQUOTE-SPLICING")?;
    let quasiquote = held_symbol(ctx, runtime, &mut held, "QUASIQUOTE")?;
    let markers = Markers {
        unquote,
        splice,
        quasiquote,
    };
    let result = qq(ctx, runtime, &mut held, 0, 1, &markers)?;
    held_get(&held, result)
}

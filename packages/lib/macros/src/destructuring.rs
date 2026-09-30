//! A shared destructuring lambda-list expander for `destructuring-bind` and
//! `defmacro`/`macrolet` argument lists (CLHS 3.4.4 and 3.4.5).
//!
//! The expander lowers a destructuring pattern into an explicit `let*` of
//! `consp`/`car`/`cdr` accesses rather than delegating to an ordinary lambda
//! list, so nested patterns, nested `&optional`/`&key` defaults, and dotted
//! tails all work without support from the front-end lambda-list lowerer
//! (which only understands flat, symbol-only ordinary lambda lists).
//!
//! `&aux` is not accepted: an occurrence is reported as a malformed pattern
//! rather than silently ignored. `&allow-other-keys` is accepted but,
//! deliberately, unknown keyword arguments are never rejected (see the
//! module-level limitation note in the crate's caller); validating them
//! would need a `tagbody`/`go` loop, and mutating a loop variable across an
//! `if` branch does not yet merge in the front end (tracked outside this
//! lane).

use ncl_object::{
    ObjectError, Runtime, ThreadContext, Word, car, cdr, string_length, string_ref, symbol_name,
};

mod held;
mod keys;
pub use held::Result;
use held::held_string;
pub use held::{held_call, held_fresh, held_get, held_push, held_raw_list, held_symbol};
use keys::walk_key;

fn symbol_text(ctx: &ThreadContext, word: Word) -> std::result::Result<String, ObjectError> {
    let name = symbol_name(ctx, word)?;
    let length = string_length(ctx, name)?;
    (0..length)
        .map(|index| string_ref(ctx, name, index))
        .collect()
}

fn is_marker(ctx: &ThreadContext, held: &[Word], word_index: usize, marker: &str) -> Result<bool> {
    let word = held_get(held, word_index)?;
    if !matches!(
        ncl_object::classify_object(ctx, word),
        ncl_object::ObjectRef::Symbol(_)
    ) {
        return Ok(false);
    }
    Ok(symbol_text(ctx, word).is_ok_and(|text| text == marker))
}

fn extract_next(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    cursor: usize,
    message: &str,
) -> Result {
    let consp = held_call(ctx, runtime, held, "CONSP", &[cursor])?;
    let element = held_call(ctx, runtime, held, "CAR", &[cursor])?;
    let text = held_string(ctx, runtime, held, message)?;
    let error_call = held_call(ctx, runtime, held, "ERROR", &[text])?;
    held_call(ctx, runtime, held, "IF", &[consp, element, error_call])
}

fn advance_past(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    cursor: usize,
    message: &str,
) -> Result {
    let consp = held_call(ctx, runtime, held, "CONSP", &[cursor])?;
    let tail = held_call(ctx, runtime, held, "CDR", &[cursor])?;
    let text = held_string(ctx, runtime, held, message)?;
    let error_call = held_call(ctx, runtime, held, "ERROR", &[text])?;
    held_call(ctx, runtime, held, "IF", &[consp, tail, error_call])
}

/// Bind `name_or_pattern` (at `held[target]`) to the value at `held[value]`,
/// recursing when the target is a nested pattern rather than a bare symbol.
fn bind_target(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    target: usize,
    value: usize,
    bindings: &mut Vec<(usize, usize)>,
) -> std::result::Result<(), ObjectError> {
    let word = held_get(held, target)?;
    if word.is_cons() || word == Word::NIL {
        let cursor = held_fresh(ctx, runtime, held)?;
        bindings.push((cursor, value));
        walk(ctx, runtime, held, target, cursor, bindings)
    } else {
        bindings.push((target, value));
        Ok(())
    }
}

/// Walk one destructuring pattern (`held[pattern]`) against the list bound
/// to the symbol at `held[cursor]`, appending `(name, value-form)` pairs to
/// `bindings` in the order a `let*` must see them.
fn walk(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    pattern: usize,
    cursor: usize,
    bindings: &mut Vec<(usize, usize)>,
) -> std::result::Result<(), ObjectError> {
    let mut remaining = pattern;
    let mut has_rest_or_key = false;
    loop {
        let word = held_get(held, remaining)?;
        if word == Word::NIL {
            break;
        }
        if !word.is_cons() {
            // A dotted tail is an implicit `&rest`.
            let rest_pattern = held_push(held, word);
            bind_target(ctx, runtime, held, rest_pattern, cursor, bindings)?;
            has_rest_or_key = true;
            break;
        }
        let element = held_push(held, car(ctx, word)?);
        let tail = held_push(held, cdr(ctx, word)?);
        if is_marker(ctx, held, element, "&OPTIONAL")? {
            walk_optional(
                ctx,
                runtime,
                held,
                tail,
                cursor,
                bindings,
                &mut has_rest_or_key,
            )?;
            break;
        }
        if is_marker(ctx, held, element, "&REST")? || is_marker(ctx, held, element, "&BODY")? {
            let (rest_pattern, after) = take_one(ctx, held, tail)?;
            bind_target(ctx, runtime, held, rest_pattern, cursor, bindings)?;
            has_rest_or_key = true;
            walk_after_rest(
                ctx,
                runtime,
                held,
                after,
                cursor,
                bindings,
                &mut has_rest_or_key,
            )?;
            break;
        }
        if is_marker(ctx, held, element, "&KEY")? {
            walk_key(ctx, runtime, held, tail, cursor, bindings)?;
            has_rest_or_key = true;
            break;
        }
        if is_marker(ctx, held, element, "&WHOLE")? {
            let (whole_name, after) = take_one(ctx, held, tail)?;
            bindings.push((whole_name, cursor));
            remaining = after;
            continue;
        }
        if is_marker(ctx, held, element, "&ENVIRONMENT")? {
            let (environment_name, after) = take_one(ctx, held, tail)?;
            let nil_value = held_push(held, Word::NIL);
            bindings.push((environment_name, nil_value));
            remaining = after;
            continue;
        }
        if is_marker(ctx, held, element, "&AUX")?
            || is_marker(ctx, held, element, "&ALLOW-OTHER-KEYS")?
        {
            return Err(ObjectError::TypeError);
        }
        let value = extract_next(
            ctx,
            runtime,
            held,
            cursor,
            "destructuring-bind: too few elements",
        )?;
        bind_target(ctx, runtime, held, element, value, bindings)?;
        let advanced = advance_past(
            ctx,
            runtime,
            held,
            cursor,
            "destructuring-bind: too few elements",
        )?;
        bindings.push((cursor, advanced));
        remaining = tail;
    }
    if !has_rest_or_key {
        let null_check = held_call(ctx, runtime, held, "NULL", &[cursor])?;
        let text = held_string(ctx, runtime, held, "destructuring-bind: too many elements")?;
        let error_call = held_call(ctx, runtime, held, "ERROR", &[text])?;
        let unless_call = held_call(ctx, runtime, held, "UNLESS", &[null_check, error_call])?;
        let ignored = held_fresh(ctx, runtime, held)?;
        bindings.push((ignored, unless_call));
    }
    Ok(())
}

/// Pop one element off a proper list held at `held[list_index]`, returning
/// `(element, rest)`.
fn take_one(
    ctx: &ThreadContext,
    held: &mut Vec<Word>,
    list_index: usize,
) -> Result<(usize, usize)> {
    let word = held_get(held, list_index)?;
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    let element = held_push(held, car(ctx, word)?);
    let rest = held_push(held, cdr(ctx, word)?);
    Ok((element, rest))
}

fn walk_after_rest(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    remaining: usize,
    cursor: usize,
    bindings: &mut Vec<(usize, usize)>,
    has_rest_or_key: &mut bool,
) -> Result {
    let word = held_get(held, remaining)?;
    if word == Word::NIL {
        return Ok(remaining);
    }
    let element = held_push(held, car(ctx, word)?);
    let tail = held_push(held, cdr(ctx, word)?);
    if is_marker(ctx, held, element, "&KEY")? {
        walk_key(ctx, runtime, held, tail, cursor, bindings)?;
        *has_rest_or_key = true;
        return Ok(tail);
    }
    Err(ObjectError::TypeError)
}

fn walk_optional(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    mut remaining: usize,
    cursor: usize,
    bindings: &mut Vec<(usize, usize)>,
    has_rest_or_key: &mut bool,
) -> Result {
    loop {
        let word = held_get(held, remaining)?;
        if word == Word::NIL {
            return Ok(remaining);
        }
        if !word.is_cons() {
            let rest_pattern = held_push(held, word);
            bind_target(ctx, runtime, held, rest_pattern, cursor, bindings)?;
            *has_rest_or_key = true;
            return Ok(held_push(held, Word::NIL));
        }
        let element = held_push(held, car(ctx, word)?);
        let tail = held_push(held, cdr(ctx, word)?);
        if is_marker(ctx, held, element, "&REST")? || is_marker(ctx, held, element, "&BODY")? {
            let (rest_pattern, after) = take_one(ctx, held, tail)?;
            bind_target(ctx, runtime, held, rest_pattern, cursor, bindings)?;
            *has_rest_or_key = true;
            return walk_after_rest(ctx, runtime, held, after, cursor, bindings, has_rest_or_key);
        }
        if is_marker(ctx, held, element, "&KEY")? {
            walk_key(ctx, runtime, held, tail, cursor, bindings)?;
            *has_rest_or_key = true;
            return Ok(tail);
        }
        // An optional spec is `name`, `(name)`, `(name default)`, or
        // `(name default supplied-p)`; `name` may itself be a pattern.
        let element_word = held_get(held, element)?;
        let (name, default, supplied) = if element_word.is_cons() {
            let (name, rest) = take_one(ctx, held, element)?;
            let rest_word = held_get(held, rest)?;
            if rest_word == Word::NIL {
                (name, None, None)
            } else {
                let (default, rest) = take_one(ctx, held, rest)?;
                let rest_word = held_get(held, rest)?;
                if rest_word == Word::NIL {
                    (name, Some(default), None)
                } else {
                    let (supplied, _) = take_one(ctx, held, rest)?;
                    (name, Some(default), Some(supplied))
                }
            }
        } else {
            (element, None, None)
        };
        let default_value = default.unwrap_or_else(|| held_push(held, Word::NIL));
        let consp = held_call(ctx, runtime, held, "CONSP", &[cursor])?;
        let current_value = held_call(ctx, runtime, held, "CAR", &[cursor])?;
        let value = held_call(
            ctx,
            runtime,
            held,
            "IF",
            &[consp, current_value, default_value],
        )?;
        bind_target(ctx, runtime, held, name, value, bindings)?;
        if let Some(supplied) = supplied {
            let consp = held_call(ctx, runtime, held, "CONSP", &[cursor])?;
            let true_symbol = held_symbol(ctx, runtime, held, "T")?;
            let nil_value = held_push(held, Word::NIL);
            let supplied_value =
                held_call(ctx, runtime, held, "IF", &[consp, true_symbol, nil_value])?;
            bindings.push((supplied, supplied_value));
        }
        let consp = held_call(ctx, runtime, held, "CONSP", &[cursor])?;
        let remaining_value = held_call(ctx, runtime, held, "CDR", &[cursor])?;
        let advanced = held_call(ctx, runtime, held, "IF", &[consp, remaining_value, cursor])?;
        bindings.push((cursor, advanced));
        remaining = tail;
    }
}

/// Expand `(destructuring-bind pattern value-form . body)`-style forms.
///
/// `pattern` may nest, may end in a dotted tail, and may use `&optional`
/// (with defaults and `supplied-p`), `&rest`/`&body`, `&key` (including
/// custom keywords, defaults, `supplied-p`, and `&allow-other-keys`),
/// `&whole`, and `&environment`. A shape mismatch signals an error instead
/// of silently binding garbage.
///
/// # Errors
/// Returns [`ObjectError`] when `pattern` is malformed.
pub fn expand_destructuring_bind(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    pattern: Word,
    value_form: Word,
    body: &[Word],
) -> std::result::Result<Word, ObjectError> {
    let mut held = vec![pattern, value_form];
    held.extend_from_slice(body);
    let body_start = 2;
    let source = held_fresh(ctx, runtime, &mut held)?;
    let mut bindings = vec![(source, 1_usize)];
    walk(ctx, runtime, &mut held, 0, source, &mut bindings)?;
    let mut pair_indexes = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let pair = [binding.0, binding.1];
        pair_indexes.push(held_raw_list(ctx, runtime, &mut held, &pair)?);
    }
    let bindings_list = held_raw_list(ctx, runtime, &mut held, &pair_indexes)?;
    let mut let_args = vec![bindings_list];
    let_args.extend(body_start..body_start + body.len());
    let result = held_call(ctx, runtime, &mut held, "LET*", &let_args)?;
    held_get(&held, result)
}

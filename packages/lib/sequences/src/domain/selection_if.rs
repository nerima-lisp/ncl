//! The `-IF`/`-IF-NOT` predicate-based sequence functions, split out of
//! `selection.rs` to keep that file under the project's line-count limit.

use ncl_object::{FunctionCaller, ObjectError, Runtime, ThreadContext, Word};

use super::{SelectionOptions, bounds, call_one, scope_roots};

/// Indices matching a one-argument predicate applied to the (optionally
/// keyed) elements of `values`, honouring `:key`, `:start`, `:end`,
/// `:from-end`, and `:count`. Backs the `-IF` and `-IF-NOT` sequence
/// functions (`FIND-IF`, `POSITION-IF`, `COUNT-IF`, `REMOVE-IF`,
/// `SUBSTITUTE-IF`, and their `-NOT` counterparts); `negate` selects the
/// `-IF-NOT` sense.
#[allow(clippy::needless_pass_by_ref_mut)]
pub fn matching_indices_if<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    predicate: Word,
    options: SelectionOptions,
    negate: bool,
) -> Result<Vec<usize>, ObjectError> {
    scope_roots(ctx, values, |ctx, roots| {
        let option_roots = [predicate, options.key.unwrap_or(Word::NIL)];
        scope_roots(ctx, &option_roots, |ctx, option_roots| {
            let predicate = *option_roots.first().ok_or(ObjectError::Layout)?;
            let key = option_roots.get(1).ok_or(ObjectError::Layout).copied()?;
            let key = (key != Word::NIL).then_some(key);
            let (start, end) = bounds(options, values.len())?;
            let mut indices = (start..end).collect::<Vec<_>>();
            if options.from_end {
                indices.reverse();
            }
            let mut found = Vec::new();
            for index in indices {
                let item = *roots.get(index).ok_or(ObjectError::Layout)?;
                let keyed = if let Some(key) = key {
                    call_one(caller, ctx, runtime, key, item)?
                } else {
                    item
                };
                let truth = call_one(caller, ctx, runtime, predicate, keyed)? != Word::NIL;
                let matched = if negate { !truth } else { truth };
                if matched {
                    found.push(index);
                    if let Some(count) = options.count
                        && found.len() >= count
                    {
                        break;
                    }
                }
            }
            Ok(found)
        })
    })
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn find_if<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    predicate: Word,
    options: SelectionOptions,
    negate: bool,
) -> Result<Word, ObjectError> {
    let Some(index) =
        matching_indices_if(ctx, runtime, caller, values, predicate, options, negate)?
            .first()
            .copied()
    else {
        return Ok(Word::NIL);
    };
    values.get(index).copied().ok_or(ObjectError::Layout)
}

pub fn position_if<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    predicate: Word,
    options: SelectionOptions,
    negate: bool,
) -> Result<Word, ObjectError> {
    let index = matching_indices_if(ctx, runtime, caller, values, predicate, options, negate)?
        .first()
        .copied();
    index.map_or(Ok(Word::NIL), |index| {
        i64::try_from(index)
            .map(Word::fixnum)
            .map_err(|_| ObjectError::TypeError)
    })
}

pub fn count_if<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    predicate: Word,
    mut options: SelectionOptions,
    negate: bool,
) -> Result<Word, ObjectError> {
    options.count = None;
    let count = i64::try_from(
        matching_indices_if(ctx, runtime, caller, values, predicate, options, negate)?.len(),
    )
    .map_err(|_| ObjectError::TypeError)?;
    Ok(Word::fixnum(count))
}

pub fn remove_if<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    predicate: Word,
    options: SelectionOptions,
    negate: bool,
) -> Result<Vec<Word>, ObjectError> {
    let indices = matching_indices_if(ctx, runtime, caller, values, predicate, options, negate)?;
    let removed = indices
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    Ok(values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (!removed.contains(&index)).then_some(*value))
        .collect())
}

#[allow(clippy::too_many_arguments)]
pub fn substitute_if<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    values: &mut [Word], // check-added-lines: allow(index) slice type
    replacement: Word,
    predicate: Word,
    options: SelectionOptions,
    negate: bool,
) -> Result<Vec<Word>, ObjectError> {
    let indices = matching_indices_if(ctx, runtime, caller, values, predicate, options, negate)?;
    let replaced = indices
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    Ok(values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if replaced.contains(&index) {
                replacement
            } else {
                *value
            }
        })
        .collect())
}

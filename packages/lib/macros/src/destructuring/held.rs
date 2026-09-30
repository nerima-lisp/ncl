//! `held`-vector helpers shared by the destructuring walk and its `&key`
//! support: every intermediate value lives in a `Vec<Word>` that gets
//! re-rooted (and refreshed) before each allocation, so a collection
//! triggered mid-expansion cannot leave a stale reference. See the crate
//! docs on `destructuring` for why this discipline is needed.

use crate::form::fresh_symbol;
use crate::{list, symbol};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word, make_string};

pub type Result<T = usize> = std::result::Result<T, ObjectError>;

pub fn held_get(held: &[Word], index: usize) -> std::result::Result<Word, ObjectError> {
    held.get(index).copied().ok_or(ObjectError::TypeError)
}

pub fn held_push(held: &mut Vec<Word>, value: Word) -> usize {
    held.push(value);
    held.len() - 1
}

pub fn held_fresh(ctx: &mut ThreadContext, runtime: &Runtime, held: &mut Vec<Word>) -> Result {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = fresh_symbol(ctx, runtime)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    Ok(held_push(held, value))
}

pub fn held_symbol(
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
    Ok(held_push(held, value))
}

pub fn held_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    text: &str,
) -> Result {
    let chars = text.chars().collect::<Vec<_>>();
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = make_string(ctx, runtime, &chars)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    Ok(held_push(held, value))
}

/// Build `(name arg*)`, interning `name` fresh each time.
pub fn held_call(
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
    Ok(held_push(held, value))
}

/// Build `(operator arg*)` where `operator` is itself a held value, such as
/// a gensym naming a `labels` function.
pub fn held_apply(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    operator: usize,
    indexes: &[usize],
) -> Result {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let mut values = Vec::with_capacity(indexes.len() + 1);
        values.push(
            roots
                .get(operator)
                .map(|root| **root)
                .ok_or(ObjectError::TypeError)?,
        );
        for index in indexes {
            values.push(
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::TypeError)?,
            );
        }
        let value = list(ctx, runtime, &values)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    Ok(held_push(held, value))
}

/// Build a literal list `(a b ...)`, unlike [`held_call`] which treats the
/// first index as an operator name to intern.
pub fn held_raw_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    indexes: &[usize],
) -> Result {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let values = indexes
            .iter()
            .map(|index| {
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::TypeError)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let value = list(ctx, runtime, &values)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    Ok(held_push(held, value))
}

use super::{
    ObjectError, Result, Runtime, ThreadContext, Word, elements, fresh_symbol, list, symbol,
};

pub(super) fn form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result {
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut operator = symbol(ctx, runtime, name)?;
        ncl_object::with_root(ctx, &mut operator, |ctx, operator| {
            let values = std::iter::once(*operator)
                .chain(roots.iter().map(|root| **root))
                .collect::<Vec<_>>();
            list(ctx, runtime, &values)
        })
    })
}

pub(super) fn held_get(held: &[Word], index: usize) -> Result<Word> {
    held.get(index).copied().ok_or(ObjectError::TypeError)
}

pub(super) fn held_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: &str,
    indexes: &[usize],
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let args = indexes
            .iter()
            .map(|index| {
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::TypeError)
            })
            .collect::<Result<Vec<_>>>()?;
        let value = form(ctx, runtime, name, &args)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

pub(super) fn held_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    indexes: &[usize],
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let values = indexes
            .iter()
            .map(|index| {
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::TypeError)
            })
            .collect::<Result<Vec<_>>>()?;
        let value = list(ctx, runtime, &values)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

pub(super) fn held_fresh_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = fresh_symbol(ctx, runtime)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

pub(super) fn expand_body(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    body: &[usize],
    end: usize,
) -> Result<Vec<usize>> {
    let finish = held_fresh_symbol(ctx, runtime, held)?;
    let mut result = Vec::with_capacity(body.len());
    for index in body {
        let word = held_get(held, *index)?;
        if word.is_cons() {
            let parts = elements(ctx, word)?;
            if parts.first().copied() == Some(held_get(held, finish)?) && parts.len() == 1 {
                result.push(held_form(ctx, runtime, held, "GO", &[end])?);
                continue;
            }
        }
        result.push(*index);
    }
    Ok(result)
}

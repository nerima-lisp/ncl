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

/// Hold the interned symbol named `name` (in `COMMON-LISP` unless `name`
/// contains a `PACKAGE::SYMBOL` separator), for generated code that must
/// refer to a specific symbol (e.g. `IT` in conditional loop clauses) rather
/// than a fresh gensym.
pub(super) fn held_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: &str,
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = symbol(ctx, runtime, name)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{elements, list, symbol};

    fn fixture() -> std::result::Result<(Runtime, ThreadContext), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        Ok((runtime, ctx))
    }

    #[test]
    fn held_builders_report_invalid_indexes_and_rewrite_return_forms()
    -> std::result::Result<(), ObjectError> {
        let (runtime, mut ctx) = fixture()?;
        let value = symbol(&mut ctx, &runtime, "VALUE")?;
        let target = symbol(&mut ctx, &runtime, "TARGET")?;
        let finish = symbol(&mut ctx, &runtime, "FINISH")?;
        let go = list(&mut ctx, &runtime, &[finish])?;
        let mut held = vec![value, target, go];
        assert_eq!(held_get(&held, 9), Err(ObjectError::TypeError));
        assert_eq!(held_form(&mut ctx, &runtime, &mut held, "CAR", &[0])?, 3);
        assert_eq!(
            elements(&mut ctx, held[3])?[0],
            symbol(&mut ctx, &runtime, "CAR")?
        );
        assert_eq!(held_list(&mut ctx, &runtime, &mut held, &[0, 1])?, 4);
        let finish_index = held_symbol(&mut ctx, &runtime, &mut held, "FINISH")?;
        let replacement = expand_body(&mut ctx, &runtime, &mut held, &[2, 0], finish_index)?;
        assert_eq!(replacement.len(), 2);
        let replacement_form = elements(&mut ctx, held[replacement[0]])?;
        assert_eq!(
            ncl_object::string_ref(&ctx, ncl_object::symbol_name(&ctx, replacement_form[0])?, 0)?,
            'F'
        );
        assert_eq!(replacement[1], 0);
        Ok(())
    }
}

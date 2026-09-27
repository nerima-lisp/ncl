use super::{form, progn};
use crate::{elements, fresh_symbol, list};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub(crate) fn binding(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    value: Word,
) -> Result {
    list(ctx, runtime, &[name, value])
}

pub(crate) fn bindings(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    pairs: &[(Word, Word)],
) -> Result {
    let values = pairs
        .iter()
        .flat_map(|&(name, value)| std::iter::once(name).chain(std::iter::once(value)))
        .collect::<Vec<_>>();
    ncl_object::with_roots(ctx, &values, |ctx, roots| {
        let mut result = Vec::with_capacity(pairs.len());
        for index in 0..pairs.len() {
            let value = ncl_object::with_roots(ctx, &result, |ctx, _result_roots| {
                let name = **roots.get(index * 2).ok_or(ObjectError::TypeError)?;
                let value = **roots.get(index * 2 + 1).ok_or(ObjectError::TypeError)?;
                binding(ctx, runtime, name, value)
            })?;
            result.push(value);
        }
        list(ctx, runtime, &result)
    })
}

pub(super) fn expand(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let clauses = elements(ctx, values.first().copied().ok_or(ObjectError::TypeError)?)?;
    let mut result = progn(ctx, runtime, values.get(1..).ok_or(ObjectError::TypeError)?)?;
    for clause in clauses.into_iter().rev() {
        let parts = elements(ctx, clause)?;
        if parts.len() != 2 {
            return Err(ObjectError::TypeError);
        }
        result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
            ncl_object::with_roots(ctx, &parts, |ctx, parts| {
                let condition = parts.first().ok_or(ObjectError::TypeError)?;
                let handler = parts.get(1).ok_or(ObjectError::TypeError)?;
                let condition = form(ctx, runtime, "QUOTE", &[**condition])?;
                let handler = form(ctx, runtime, "FUNCTION", &[**handler])?;
                let push = form(ctx, runtime, "NCL-EXT::PUSH-HANDLER", &[condition, handler])?;
                let token = fresh_symbol(ctx, runtime)?;
                let bindings = bindings(ctx, runtime, &[(token, push)])?;
                let pop = form(ctx, runtime, "NCL-EXT::POP-HANDLER", &[token])?;
                let body = form(ctx, runtime, "PROGN", &[*result, pop])?;
                ncl_object::with_roots(ctx, &[bindings, body], |ctx, roots| {
                    let bindings = roots.first().ok_or(ObjectError::Layout)?;
                    let body = roots.get(1).ok_or(ObjectError::Layout)?;
                    form(ctx, runtime, "LET", &[**bindings, **body])
                })
            })
        })?;
    }
    Ok(result)
}

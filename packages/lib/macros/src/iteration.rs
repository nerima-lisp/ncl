//! Expanders for destructuring and simple iteration macros.
#![allow(clippy::redundant_pub_crate)]

use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word, symbol_name,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

fn held_get(held: &[Word], index: usize) -> Result<Word> {
    held.get(index).copied().ok_or(ObjectError::TypeError)
}

fn form(ctx: &mut ThreadContext, runtime: &Runtime, name: &str, args: &[Word]) -> Result {
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let operator = symbol(ctx, runtime, name)?;
        let mut values = Vec::with_capacity(roots.len() + 1);
        values.push(operator);
        values.extend(roots.iter().map(|value| **value));
        list(ctx, runtime, &values)
    })
}

fn held_form(
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

fn held_list(
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

fn held_fresh_symbol(
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

fn destructuring_bind_call(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let pattern = values.first().copied().ok_or(ObjectError::TypeError)?;
    let value_form = values.get(1).copied().ok_or(ObjectError::TypeError)?;
    let body = values.get(2..).ok_or(ObjectError::TypeError)?;
    crate::destructuring::expand_destructuring_bind(ctx, runtime, pattern, value_form, body)
}

fn dolist(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        if !(2..=3).contains(&spec.len()) {
            return Err(ObjectError::TypeError);
        }
        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let variable = held.len();
        held.push(spec.first().copied().ok_or(ObjectError::TypeError)?);
        symbol_name(ctx, held_get(&held, variable)?)?;
        let list_form = held.len();
        held.push(spec.get(1).copied().ok_or(ObjectError::TypeError)?);
        let result = held.len();
        held.push(spec.get(2).copied().unwrap_or(Word::NIL));
        let cursor = held_fresh_symbol(ctx, runtime, &mut held)?;
        let loop_tag = held_fresh_symbol(ctx, runtime, &mut held)?;
        let end_tag = held_fresh_symbol(ctx, runtime, &mut held)?;
        let endp = held_form(ctx, runtime, &mut held, "ENDP", &[cursor])?;
        let go_end = held_form(ctx, runtime, &mut held, "GO", &[end_tag])?;
        let test = held_form(ctx, runtime, &mut held, "IF", &[endp, go_end])?;
        let car = held_form(ctx, runtime, &mut held, "CAR", &[cursor])?;
        let set_variable = held_form(ctx, runtime, &mut held, "SETQ", &[variable, car])?;
        let cdr = held_form(ctx, runtime, &mut held, "CDR", &[cursor])?;
        let advance = held_form(ctx, runtime, &mut held, "SETQ", &[cursor, cdr])?;
        let jump = held_form(ctx, runtime, &mut held, "GO", &[loop_tag])?;
        let mut tagbody = vec![loop_tag, test, set_variable];
        tagbody.extend(1..values.len());
        tagbody.extend([advance, jump, end_tag]);
        let tagbody = held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?;
        let nil = held.len();
        held.push(Word::NIL);
        let clear = held_form(ctx, runtime, &mut held, "SETQ", &[variable, nil])?;
        let tagbody = held_form(ctx, runtime, &mut held, "PROGN", &[tagbody, clear])?;
        let variable_binding = held_list(ctx, runtime, &mut held, &[variable, result])?;
        let cursor_binding = held_list(ctx, runtime, &mut held, &[cursor, list_form])?;
        let bindings = held_list(ctx, runtime, &mut held, &[variable_binding, cursor_binding])?;
        let body = held_form(ctx, runtime, &mut held, "LET", &[bindings, tagbody, result])?;
        let block_name = held.len();
        held.push(Word::NIL);
        let block = held_form(ctx, runtime, &mut held, "BLOCK", &[block_name, body])?;
        held_get(&held, block)
    })
}

fn dotimes(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        if !(2..=3).contains(&spec.len()) {
            return Err(ObjectError::TypeError);
        }
        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let variable = held.len();
        held.push(spec.first().copied().ok_or(ObjectError::TypeError)?);
        symbol_name(ctx, held_get(&held, variable)?)?;
        let limit_form = held.len();
        held.push(spec.get(1).copied().ok_or(ObjectError::TypeError)?);
        let result = held.len();
        held.push(spec.get(2).copied().unwrap_or(Word::NIL));
        let limit = held_fresh_symbol(ctx, runtime, &mut held)?;
        let loop_tag = held_fresh_symbol(ctx, runtime, &mut held)?;
        let end_tag = held_fresh_symbol(ctx, runtime, &mut held)?;
        let comparison = held_form(ctx, runtime, &mut held, ">=", &[variable, limit])?;
        let go_end = held_form(ctx, runtime, &mut held, "GO", &[end_tag])?;
        let test = held_form(ctx, runtime, &mut held, "IF", &[comparison, go_end])?;
        let one = held.len();
        held.push(Word::fixnum(1));
        let next = held_form(ctx, runtime, &mut held, "+", &[variable, one])?;
        let increment = held_form(ctx, runtime, &mut held, "SETQ", &[variable, next])?;
        let jump = held_form(ctx, runtime, &mut held, "GO", &[loop_tag])?;
        let mut tagbody = vec![loop_tag, test];
        tagbody.extend(1..values.len());
        tagbody.extend([increment, jump, end_tag]);
        let tagbody = held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?;
        let zero = held.len();
        held.push(Word::fixnum(0));
        let variable_binding = held_list(ctx, runtime, &mut held, &[variable, zero])?;
        let limit_binding = held_list(ctx, runtime, &mut held, &[limit, limit_form])?;
        let bindings = held_list(ctx, runtime, &mut held, &[variable_binding, limit_binding])?;
        let body = held_form(ctx, runtime, &mut held, "LET", &[bindings, tagbody, result])?;
        let block_name = held.len();
        held.push(Word::NIL);
        let block = held_form(ctx, runtime, &mut held, "BLOCK", &[block_name, body])?;
        held_get(&held, block)
    })
}

fn args(ctx: &mut ThreadContext, form_word: Word) -> Result<Vec<Word>> {
    let mut values = elements(ctx, form_word)?;
    if values.is_empty() {
        return Err(ObjectError::TypeError);
    }
    values.remove(0);
    Ok(values)
}

macro_rules! callbacks {
    ($($legacy:ident, $adapter:ident, $expand:ident);+ $(;)?) => { $(
        fn $legacy(runtime: &Runtime, ctx: &mut ThreadContext, input: &[Word], _: &mut MultipleValues) -> Result {
            let values = args(ctx, input.first().copied().ok_or(ObjectError::TypeError)?)?;
            $expand(ctx, runtime, &values)
        }
        pub fn $adapter(ctx: &mut ThreadContext, runtime: &Runtime, input: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result {
            let words = (0..input.len()).filter_map(|index| input.get(index)).collect::<Vec<_>>();
            $legacy(runtime, ctx, &words, values)
        }
    )+ }
}

callbacks! {
    expand_destructuring_bind, expand_destructuring_bind_adapter, destructuring_bind_call;
    expand_dolist, expand_dolist_adapter, dolist;
    expand_dotimes, expand_dotimes_adapter, dotimes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{elements, list, symbol};

    #[test]
    fn iteration_adapters_expand_valid_forms_and_reject_missing_arguments()
    -> std::result::Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let mut values = MultipleValues::new();
        let empty = BuiltinArgs::new(&[]);
        assert_eq!(
            expand_dolist_adapter(&mut ctx, &runtime, &empty, &mut values),
            Err(ObjectError::TypeError)
        );

        let x = symbol(&mut ctx, &runtime, "X")?;
        let list_value = list(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)])?;
        let spec = list(&mut ctx, &runtime, &[x, list_value, Word::fixnum(9)])?;
        let body = symbol(&mut ctx, &runtime, "BODY")?;
        let dolist = symbol(&mut ctx, &runtime, "DOLIST")?;
        let form = list(&mut ctx, &runtime, &[dolist, spec, body])?;
        let expansion =
            expand_dolist_adapter(&mut ctx, &runtime, &BuiltinArgs::new(&[form]), &mut values)?;
        assert_eq!(
            elements(&mut ctx, expansion)?[0],
            symbol(&mut ctx, &runtime, "BLOCK")?
        );

        let count = list(&mut ctx, &runtime, &[x, Word::fixnum(3), Word::fixnum(7)])?;
        let dotimes = symbol(&mut ctx, &runtime, "DOTIMES")?;
        let form = list(&mut ctx, &runtime, &[dotimes, count, body])?;
        let expansion =
            expand_dotimes_adapter(&mut ctx, &runtime, &BuiltinArgs::new(&[form]), &mut values)?;
        assert!(elements(&mut ctx, expansion)?.len() >= 3);
        assert_eq!(held_get(&[], 0), Err(ObjectError::TypeError));
        Ok(())
    }
}

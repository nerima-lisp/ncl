//! Expanders for destructuring and simple iteration macros.
#![allow(clippy::redundant_pub_crate)]

use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word, symbol_name,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

fn form(ctx: &mut ThreadContext, runtime: &Runtime, name: &str, args: &[Word]) -> Result {
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let operator = symbol(ctx, runtime, name)?;
        let mut values = Vec::with_capacity(roots.len() + 1);
        values.push(operator);
        values.extend(roots.iter().map(|value| **value));
        list(ctx, runtime, &values)
    })
}

fn binding(ctx: &mut ThreadContext, runtime: &Runtime, name: Word, value: Word) -> Result {
    list(ctx, runtime, &[name, value])
}

fn progn(ctx: &mut ThreadContext, runtime: &Runtime, body: &[Word]) -> Result {
    form(ctx, runtime, "PROGN", body)
}

fn destructuring_bind_call(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let lambda_list = values.first().copied().ok_or(ObjectError::TypeError)?;
    elements(ctx, lambda_list)?;
    let value = values.get(1).copied().ok_or(ObjectError::TypeError)?;
    let body = progn(ctx, runtime, values.get(2..).ok_or(ObjectError::TypeError)?)?;
    ncl_object::with_roots(ctx, &[lambda_list, body, value], |ctx, roots| {
        let lambda = form(
            ctx,
            runtime,
            "LAMBDA",
            &[
                **roots.first().ok_or(ObjectError::TypeError)?,
                **roots.get(1).ok_or(ObjectError::TypeError)?,
            ],
        )?;
        ncl_object::with_root(ctx, &mut lambda.clone(), |ctx, lambda| {
            form(
                ctx,
                runtime,
                "FUNCALL",
                &[*lambda, **roots.get(2).ok_or(ObjectError::TypeError)?],
            )
        })
    })
}

fn dolist(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let spec = elements(ctx, values.first().copied().ok_or(ObjectError::TypeError)?)?;
    if !(2..=3).contains(&spec.len()) {
        return Err(ObjectError::TypeError);
    }
    let variable = spec[0];
    symbol_name(ctx, variable)?;
    let list_form = spec[1];
    let result = spec.get(2).copied().unwrap_or(Word::NIL);
    let cursor = fresh_symbol(ctx, runtime)?;
    let loop_tag = fresh_symbol(ctx, runtime)?;
    let end_tag = fresh_symbol(ctx, runtime)?;
    let endp = form(ctx, runtime, "ENDP", &[cursor])?;
    let go_end = form(ctx, runtime, "GO", &[end_tag])?;
    let test = form(ctx, runtime, "IF", &[endp, go_end])?;
    let car = form(ctx, runtime, "CAR", &[cursor])?;
    let set_variable = form(ctx, runtime, "SETQ", &[variable, car])?;
    let cdr = form(ctx, runtime, "CDR", &[cursor])?;
    let advance = form(ctx, runtime, "SETQ", &[cursor, cdr])?;
    let jump = form(ctx, runtime, "GO", &[loop_tag])?;
    let mut tagbody = vec![loop_tag, test, set_variable];
    tagbody.extend_from_slice(values.get(1..).ok_or(ObjectError::TypeError)?);
    tagbody.extend([advance, jump, end_tag]);
    let tagbody = form(ctx, runtime, "TAGBODY", &tagbody)?;
    let clear = form(ctx, runtime, "SETQ", &[variable, Word::NIL])?;
    let tagbody = form(ctx, runtime, "PROGN", &[tagbody, clear])?;
    let variable_binding = binding(ctx, runtime, variable, Word::NIL)?;
    let cursor_binding = binding(ctx, runtime, cursor, list_form)?;
    let bindings = list(ctx, runtime, &[variable_binding, cursor_binding])?;
    let body = form(ctx, runtime, "LET", &[bindings, tagbody, result])?;
    form(ctx, runtime, "BLOCK", &[Word::NIL, body])
}

fn dotimes(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let spec = elements(ctx, values.first().copied().ok_or(ObjectError::TypeError)?)?;
    if !(2..=3).contains(&spec.len()) {
        return Err(ObjectError::TypeError);
    }
    let variable = spec[0];
    symbol_name(ctx, variable)?;
    let limit_form = spec[1];
    let result = spec.get(2).copied().unwrap_or(Word::NIL);
    let limit = fresh_symbol(ctx, runtime)?;
    let loop_tag = fresh_symbol(ctx, runtime)?;
    let end_tag = fresh_symbol(ctx, runtime)?;
    let comparison = form(ctx, runtime, ">=", &[variable, limit])?;
    let go_end = form(ctx, runtime, "GO", &[end_tag])?;
    let test = form(ctx, runtime, "IF", &[comparison, go_end])?;
    let next = form(ctx, runtime, "+", &[variable, Word::fixnum(1)])?;
    let increment = form(ctx, runtime, "SETQ", &[variable, next])?;
    let jump = form(ctx, runtime, "GO", &[loop_tag])?;
    let mut tagbody = vec![loop_tag, test];
    tagbody.extend_from_slice(values.get(1..).ok_or(ObjectError::TypeError)?);
    tagbody.extend([increment, jump, end_tag]);
    let tagbody = form(ctx, runtime, "TAGBODY", &tagbody)?;
    let variable_binding = binding(ctx, runtime, variable, Word::fixnum(0))?;
    let limit_binding = binding(ctx, runtime, limit, limit_form)?;
    let bindings = list(ctx, runtime, &[variable_binding, limit_binding])?;
    let body = form(ctx, runtime, "LET", &[bindings, tagbody, result])?;
    form(ctx, runtime, "BLOCK", &[Word::NIL, body])
}

fn args(ctx: &mut ThreadContext, form_word: Word) -> Result<Vec<Word>> {
    let mut values = elements(ctx, form_word)?;
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

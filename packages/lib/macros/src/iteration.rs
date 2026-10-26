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

fn held_named_symbol(
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

fn symbols_loop(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    helper: &str,
    mode: &str,
    all_packages: bool,
) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        let spec_max = if all_packages { 2 } else { 3 };
        if !(1..=spec_max).contains(&spec.len()) {
            return Err(ObjectError::TypeError);
        }

        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let variable = held.len();
        held.push(spec[0]);
        symbol_name(ctx, held_get(&held, variable)?)?;

        let package = if all_packages {
            held.len()
        } else if let Some(package) = spec.get(1) {
            let index = held.len();
            held.push(*package);
            index
        } else {
            held_named_symbol(ctx, runtime, &mut held, "COMMON-LISP::*PACKAGE*")?
        };
        if all_packages {
            held.push(Word::NIL);
        }

        let result = held.len();
        held.push(if all_packages {
            spec.get(1).copied().unwrap_or(Word::NIL)
        } else {
            spec.get(2).copied().unwrap_or(Word::NIL)
        });
        let kind = held_named_symbol(
            ctx,
            runtime,
            &mut held,
            match mode {
                "ALL" => "KEYWORD::ALL",
                "EXTERNAL" => "KEYWORD::EXTERNAL",
                _ => "KEYWORD::SYMBOLS",
            },
        )?;
        let symbols = held_form(ctx, runtime, &mut held, helper, &[kind, package])?;
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

        let declare = held_named_symbol(ctx, runtime, &mut held, "DECLARE")?;
        let mut body_start = 1;
        let mut declarations = Vec::new();
        while let Some(body_form) = roots.get(body_start).map(|root| **root) {
            if !body_form.is_cons() || ncl_object::car(ctx, body_form)? != held_get(&held, declare)?
            {
                break;
            }
            declarations.push(body_start);
            body_start += 1;
        }

        let mut tagbody = vec![loop_tag, test, set_variable];
        tagbody.extend(body_start..roots.len());
        tagbody.extend([advance, jump, end_tag]);
        let tagbody = held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?;
        let nil = held.len();
        held.push(Word::NIL);
        let clear = held_form(ctx, runtime, &mut held, "SETQ", &[variable, nil])?;
        let variable_binding = held_list(ctx, runtime, &mut held, &[variable, nil])?;
        let cursor_binding = held_list(ctx, runtime, &mut held, &[cursor, symbols])?;
        let bindings = held_list(ctx, runtime, &mut held, &[variable_binding, cursor_binding])?;
        let mut body = vec![bindings];
        body.extend(declarations);
        body.extend([tagbody, clear, result]);
        let body = held_form(ctx, runtime, &mut held, "LET", &body)?;
        let block_name = held.len();
        held.push(Word::NIL);
        let block = held_form(ctx, runtime, &mut held, "BLOCK", &[block_name, body])?;
        held_get(&held, block)
    })
}

fn expand_do_symbols(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    symbols_loop(
        ctx,
        runtime,
        values,
        "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS",
        "SYMBOLS",
        false,
    )
}

fn expand_do_external_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    symbols_loop(
        ctx,
        runtime,
        values,
        "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS",
        "EXTERNAL",
        false,
    )
}

fn expand_do_all_symbols(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    symbols_loop(
        ctx,
        runtime,
        values,
        "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS",
        "ALL",
        true,
    )
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
    expand_dotimes, expand_dotimes_adapter, dotimes;
    do_symbols, expand_do_symbols_adapter, expand_do_symbols;
    do_external_symbols, expand_do_external_symbols_adapter, expand_do_external_symbols;
    do_all_symbols, expand_do_all_symbols_adapter, expand_do_all_symbols
}

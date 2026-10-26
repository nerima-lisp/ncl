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
            // check-added-lines: allow(index) held indexes come from prior held pushes.
            .map(|index| roots[*index].get())
            .collect::<Vec<_>>();
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
            // check-added-lines: allow(index) held indexes come from prior held pushes.
            .map(|index| roots[*index].get())
            .collect::<Vec<_>>();
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

fn held_fresh_symbols(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    count: usize,
) -> Result<Vec<usize>> {
    (0..count)
        .map(|_| held_fresh_symbol(ctx, runtime, held))
        .collect()
}

fn held_forms(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    forms: &[(&str, &[usize])],
) -> Result<Vec<usize>> {
    forms
        .iter()
        .map(|(name, indexes)| held_form(ctx, runtime, held, name, indexes))
        .collect()
}

fn held_lists(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    lists: &[&[usize]],
) -> Result<Vec<usize>> {
    lists
        .iter()
        .map(|indexes| held_list(ctx, runtime, held, indexes))
        .collect()
}

fn destructuring_bind_call(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let pattern = values.first().copied().ok_or(ObjectError::TypeError)?;
    let value_form = values.get(1).copied().ok_or(ObjectError::TypeError)?;
    // check-added-lines: allow(index) get(1) above proves at least two values.
    let body = &values[2..];
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
        held.push(spec[0]); // check-added-lines: allow(index) spec length was validated above.
        symbol_name(ctx, held[variable])?;
        let list_form = held.len();
        held.push(spec[1]); // check-added-lines: allow(index) spec length was validated above.
        let result = held.len();
        held.push(spec.get(2).copied().unwrap_or(Word::NIL));
        let fresh = held_fresh_symbols(ctx, runtime, &mut held, 3)?;
        // check-added-lines: allow(index) the helper returns the requested three symbols.
        let (cursor, loop_tag, end_tag) = (fresh[0], fresh[1], fresh[2]);
        let cursor_arg = [cursor];
        let end_tag_arg = [end_tag];
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[("ENDP", &cursor_arg), ("GO", &end_tag_arg)],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (endp, go_end) = (forms[0], forms[1]);
        let test_args: [usize; 2] = (endp, go_end).into();
        let test = held_form(ctx, runtime, &mut held, "IF", &test_args)?;
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[("CAR", &cursor_arg), ("CDR", &cursor_arg)],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (car, cdr) = (forms[0], forms[1]);
        let set_variable_args = [variable, car];
        let advance_args = [cursor, cdr];
        let jump_args = [loop_tag];
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[
                ("SETQ", &set_variable_args),
                ("SETQ", &advance_args),
                ("GO", &jump_args),
            ],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (set_variable, advance, jump) = (forms[0], forms[1], forms[2]);
        let mut tagbody = vec![loop_tag, test, set_variable];
        tagbody.extend(1..values.len());
        tagbody.extend([advance, jump, end_tag]);
        let tagbody = held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?;
        let nil = held.len();
        held.push(Word::NIL);
        let clear = held_form(ctx, runtime, &mut held, "SETQ", &[variable, nil])?;
        let tagbody = held_form(ctx, runtime, &mut held, "PROGN", &[tagbody, clear])?;
        let variable_binding_args = [variable, result];
        let cursor_binding_args = [cursor, list_form];
        let bindings = held_lists(
            ctx,
            runtime,
            &mut held,
            &[&variable_binding_args, &cursor_binding_args],
        )?;
        // check-added-lines: allow(index) held_lists returns one result per requested list.
        let (variable_binding, cursor_binding) = (bindings[0], bindings[1]);
        let binding_indexes: [usize; 2] = (variable_binding, cursor_binding).into();
        let bindings = held_list(ctx, runtime, &mut held, &binding_indexes)?;
        let body = held_form(ctx, runtime, &mut held, "LET", &[bindings, tagbody, result])?;
        let block_name = held.len();
        held.push(Word::NIL);
        let block = held_form(ctx, runtime, &mut held, "BLOCK", &[block_name, body])?;
        Ok(held[block]) // check-added-lines: allow(index) held_form returns the appended final slot.
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
        held.push(spec[0]); // check-added-lines: allow(index) spec length was validated above.
        symbol_name(ctx, held[variable])?; // check-added-lines: allow(index) variable was just appended.
        let limit_form = held.len();
        held.push(spec[1]); // check-added-lines: allow(index) spec length was validated above.
        let result = held.len();
        held.push(spec.get(2).copied().unwrap_or(Word::NIL));
        let fresh = held_fresh_symbols(ctx, runtime, &mut held, 3)?;
        // check-added-lines: allow(index) the helper returns the requested three symbols.
        let (limit, loop_tag, end_tag) = (fresh[0], fresh[1], fresh[2]);
        let comparison_args = [variable, limit];
        let end_tag_arg = [end_tag];
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[(">=", &comparison_args), ("GO", &end_tag_arg)],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (comparison, go_end) = (forms[0], forms[1]);
        let test_args: [usize; 2] = (comparison, go_end).into();
        let test = held_form(ctx, runtime, &mut held, "IF", &test_args)?;
        let one = held.len();
        held.push(Word::fixnum(1));
        let next_args = [variable, one];
        let loop_tag_arg = [loop_tag];
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[("+", &next_args), ("GO", &loop_tag_arg)],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (next, jump) = (forms[0], forms[1]);
        let increment = held_form(ctx, runtime, &mut held, "SETQ", &[variable, next])?;
        let mut tagbody = vec![loop_tag, test];
        tagbody.extend(1..values.len());
        tagbody.extend([increment, jump, end_tag]);
        let tagbody = held_form(ctx, runtime, &mut held, "TAGBODY", &tagbody)?;
        let zero = held.len();
        held.push(Word::fixnum(0));
        let variable_binding_args = [variable, zero];
        let limit_binding_args = [limit, limit_form];
        let bindings = held_lists(
            ctx,
            runtime,
            &mut held,
            &[&variable_binding_args, &limit_binding_args],
        )?;
        // check-added-lines: allow(index) held_lists returns one result per requested list.
        let (variable_binding, limit_binding) = (bindings[0], bindings[1]);
        let binding_indexes: [usize; 2] = (variable_binding, limit_binding).into();
        let bindings = held_list(ctx, runtime, &mut held, &binding_indexes)?;
        let body = held_form(ctx, runtime, &mut held, "LET", &[bindings, tagbody, result])?;
        let block_name = held.len();
        held.push(Word::NIL);
        let block = held_form(ctx, runtime, &mut held, "BLOCK", &[block_name, body])?;
        Ok(held[block]) // check-added-lines: allow(index) held_form returns the appended final slot.
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

#[derive(Clone, Copy)]
enum SymbolIterationMode {
    All,
    External,
    Symbols,
}

impl SymbolIterationMode {
    const fn keyword(self) -> &'static str {
        match self {
            Self::All => "KEYWORD::ALL",
            Self::External => "KEYWORD::EXTERNAL",
            Self::Symbols => "KEYWORD::SYMBOLS",
        }
    }
}

fn symbols_loop(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    helper: &str,
    mode: SymbolIterationMode,
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
        held.push(spec[0]); // check-added-lines: allow(index) spec length was validated above.
        symbol_name(ctx, held[variable])?; // check-added-lines: allow(index) variable was just appended.

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
        let kind = held_named_symbol(ctx, runtime, &mut held, mode.keyword())?;
        let symbols = held_form(ctx, runtime, &mut held, helper, &[kind, package])?;
        let fresh = held_fresh_symbols(ctx, runtime, &mut held, 3)?;
        // check-added-lines: allow(index) the helper returns the requested three symbols.
        let (cursor, loop_tag, end_tag) = (fresh[0], fresh[1], fresh[2]);
        let cursor_arg = [cursor];
        let end_tag_arg = [end_tag];
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[("ENDP", &cursor_arg), ("GO", &end_tag_arg)],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (endp, go_end) = (forms[0], forms[1]);
        let test_args: [usize; 2] = (endp, go_end).into();
        let test = held_form(ctx, runtime, &mut held, "IF", &test_args)?;
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[("CAR", &cursor_arg), ("CDR", &cursor_arg)],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (car, cdr) = (forms[0], forms[1]);
        let set_variable_args = [variable, car];
        let advance_args = [cursor, cdr];
        let jump_args = [loop_tag];
        let forms = held_forms(
            ctx,
            runtime,
            &mut held,
            &[
                ("SETQ", &set_variable_args),
                ("SETQ", &advance_args),
                ("GO", &jump_args),
            ],
        )?;
        // check-added-lines: allow(index) held_forms returns one result per requested form.
        let (set_variable, advance, jump) = (forms[0], forms[1], forms[2]);

        let declare = held_named_symbol(ctx, runtime, &mut held, "DECLARE")?;
        let mut body_start = 1;
        let mut declarations = Vec::new();
        while let Some(body_form) = roots.get(body_start).map(|root| **root) {
            let declaration = held[declare]; // check-added-lines: allow(index) declare was just appended.
            if !body_form.is_cons() || ncl_object::car(ctx, body_form)? != declaration {
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
        Ok(held[block]) // check-added-lines: allow(index) held_form returns the appended final slot.
    })
}

fn expand_do_symbols(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    symbols_loop(
        ctx,
        runtime,
        values,
        "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS",
        SymbolIterationMode::Symbols,
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
        SymbolIterationMode::External,
        false,
    )
}

fn expand_do_all_symbols(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    symbols_loop(
        ctx,
        runtime,
        values,
        "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS",
        SymbolIterationMode::All,
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
            let form = input.first().copied().ok_or(ObjectError::TypeError)?;
            let values = args(ctx, form)?;
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

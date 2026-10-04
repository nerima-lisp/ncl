//! Structural expanders for the ANSI pretty-printer local macros.

use crate::{elements, list, symbol};
use ncl_object::{
    BuiltinArgs, LispError, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object,
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

fn is_keyword(ctx: &ThreadContext, runtime: &Runtime, word: Word, name: &str) -> Result<bool> {
    let ObjectRef::Symbol(symbol_ref) = classify_object(ctx, word) else {
        return Ok(false);
    };
    let Some(keyword) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(false);
    };
    if ncl_object::symbol_package(ctx, symbol_ref)? != keyword {
        return Ok(false);
    }
    let actual = ncl_object::symbol_name(ctx, symbol_ref)?;
    let actual = (0..ncl_object::string_length(ctx, actual)?)
        .map(|index| ncl_object::string_ref(ctx, actual, index))
        .collect::<std::result::Result<String, _>>()?;
    Ok(actual == name)
}

fn is_keyword_symbol(ctx: &ThreadContext, runtime: &Runtime, word: Word) -> Result<bool> {
    let ObjectRef::Symbol(symbol_ref) = classify_object(ctx, word) else {
        return Ok(false);
    };
    let Some(keyword) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(false);
    };
    Ok(ncl_object::symbol_package(ctx, symbol_ref)? == keyword)
}

fn stream_form(ctx: &mut ThreadContext, runtime: &Runtime, stream: Word) -> Result {
    if stream == Word::TRUE {
        symbol(ctx, runtime, "*TERMINAL-IO*")
    } else if stream == Word::NIL {
        symbol(ctx, runtime, "*STANDARD-OUTPUT*")
    } else {
        Ok(stream)
    }
}

const fn program_error(ctx: &mut ThreadContext) -> ObjectError {
    ctx.set_pending_lisp_error(LispError::ProgramError(
        ncl_object::ProgramError::UnknownKeyword,
    ));
    ObjectError::TypeError
}

/// Expand `PPRINT-LOGICAL-BLOCK` into the printer bridge and its lexical local
/// macros. The bridge receives a thunk so the printer can establish its state
/// before `PPRINT-POP` or `PPRINT-EXIT-IF-LIST-EXHAUSTED` is expanded/executed.
fn expand_logical_block(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        if spec.len() < 2 {
            return Err(ObjectError::TypeError);
        }
        let (stream_word, object, keyword_values) = match spec.as_slice() {
            [stream, object, rest @ ..] => (*stream, *object, rest),
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) slice shape validation
        };
        let stream = stream_form(ctx, runtime, stream_word)?;
        let mut prefix = Word::NIL;
        let mut per_line_prefix = Word::NIL;
        let mut suffix = Word::NIL;
        let mut pairs = keyword_values.chunks(2);
        for pair in &mut pairs {
            // check-added-lines: allow(index) fixed pair destructuring
            let [key, value] = pair else {
                return Err(ObjectError::TypeError);
            };
            let key = *key;
            let value = *value;
            if is_keyword(ctx, runtime, key, "PREFIX")? {
                if prefix != Word::NIL {
                    return Err(ObjectError::TypeError);
                }
                prefix = value;
            } else if is_keyword(ctx, runtime, key, "PER-LINE-PREFIX")? {
                if per_line_prefix != Word::NIL {
                    return Err(ObjectError::TypeError);
                }
                per_line_prefix = value;
            } else if is_keyword(ctx, runtime, key, "SUFFIX")? {
                if suffix != Word::NIL {
                    return Err(ObjectError::TypeError);
                }
                suffix = value;
            } else if is_keyword_symbol(ctx, runtime, key)? {
                return Err(program_error(ctx));
            } else {
                return Err(ObjectError::TypeError);
            }
        }

        let body_values = roots.iter().skip(1).map(|root| **root).collect::<Vec<_>>();
        let body = form(ctx, runtime, "PROGN", &body_values)?;
        let pop_name = symbol(ctx, runtime, "PPRINT-POP")?;
        let exit_name = symbol(ctx, runtime, "PPRINT-EXIT-IF-LIST-EXHAUSTED")?;
        let pop_call = form(ctx, runtime, "NCL-EXT::PPRINT-POP", &[])?;
        let exit_call = form(ctx, runtime, "NCL-EXT::PPRINT-EXIT-IF-LIST-EXHAUSTED", &[])?;
        let pop_body = form(ctx, runtime, "QUOTE", &[pop_call])?;
        let exit_body = form(ctx, runtime, "QUOTE", &[exit_call])?;
        let pop_definition = list(ctx, runtime, &[pop_name, Word::NIL, pop_body])?;
        let exit_definition = list(ctx, runtime, &[exit_name, Word::NIL, exit_body])?;
        let definitions = list(ctx, runtime, &[pop_definition, exit_definition])?;
        let macrolet = form(ctx, runtime, "MACROLET", &[definitions, body])?;
        let thunk_params = list(ctx, runtime, &[])?;
        let thunk = form(ctx, runtime, "LAMBDA", &[thunk_params, macrolet])?;
        form(
            ctx,
            runtime,
            "NCL-EXT::PPRINT-LOGICAL-BLOCK",
            &[stream, object, prefix, per_line_prefix, suffix, thunk],
        )
    })
}

fn expand_pop(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    if !values.is_empty() {
        return Err(ObjectError::TypeError);
    }
    form(ctx, runtime, "NCL-EXT::PPRINT-POP", &[])
}

fn expand_exit_if_list_exhausted(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    if !values.is_empty() {
        return Err(ObjectError::TypeError);
    }
    form(ctx, runtime, "NCL-EXT::PPRINT-EXIT-IF-LIST-EXHAUSTED", &[])
}

pub fn expand_logical_block_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.get(0).ok_or(ObjectError::TypeError)?;
    let values = crate::macro_arguments(ctx, form)?;
    expand_logical_block(ctx, runtime, &values)
}

pub fn expand_pop_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.get(0).ok_or(ObjectError::TypeError)?;
    let values = crate::macro_arguments(ctx, form)?;
    expand_pop(ctx, runtime, &values)
}

pub fn expand_exit_if_list_exhausted_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.get(0).ok_or(ObjectError::TypeError)?;
    let values = crate::macro_arguments(ctx, form)?;
    expand_exit_if_list_exhausted(ctx, runtime, &values)
}

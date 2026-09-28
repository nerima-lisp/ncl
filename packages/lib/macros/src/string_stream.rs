//! Expanders for the string stream convenience macros.

use crate::{elements, list, symbol};
use ncl_object::{
    BuiltinArgs, LispError, MultipleValues, ObjectError, ObjectRef, ProgramError, Runtime,
    ThreadContext, Word, classify_object, symbol_package,
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

fn is_keyword(ctx: &ThreadContext, runtime: &Runtime, word: Word, expected: &str) -> Result<bool> {
    let ObjectRef::Symbol(symbol) = classify_object(ctx, word) else {
        return Ok(false);
    };
    let Some(keyword_package) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(false);
    };
    if symbol_package(ctx, symbol)? != keyword_package {
        return Ok(false);
    }
    let name = ncl_object::symbol_name(ctx, symbol)?;
    let actual = (0..ncl_object::string_length(ctx, name)?)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect::<std::result::Result<String, _>>()?;
    Ok(actual == expected)
}

fn is_keyword_symbol(ctx: &ThreadContext, runtime: &Runtime, word: Word) -> Result<bool> {
    let ObjectRef::Symbol(symbol) = classify_object(ctx, word) else {
        return Ok(false);
    };
    let Some(keyword_package) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(false);
    };
    Ok(symbol_package(ctx, symbol)? == keyword_package)
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

fn expand_output(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        if spec.len() != 1 {
            return Err(ObjectError::TypeError);
        }
        let variable = spec.first().copied().ok_or(ObjectError::Layout)?;
        if !matches!(classify_object(ctx, variable), ObjectRef::Symbol(_)) {
            return Err(ObjectError::TypeError);
        }

        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let body_end = held.len();
        let variable_index = held.len();
        held.push(variable);
        let stream = held_form(ctx, runtime, &mut held, "MAKE-STRING-OUTPUT-STREAM", &[])?;
        let binding = held_list(ctx, runtime, &mut held, &[variable_index, stream])?;
        let bindings = held_list(ctx, runtime, &mut held, &[binding])?;
        let body = held_form(
            ctx,
            runtime,
            &mut held,
            "PROGN",
            &(1..body_end).collect::<Vec<_>>(),
        )?;
        let output = held_form(
            ctx,
            runtime,
            &mut held,
            "GET-OUTPUT-STREAM-STRING",
            &[variable_index],
        )?;
        let result = held_form(ctx, runtime, &mut held, "PROGN", &[body, output])?;
        let expansion = held_form(ctx, runtime, &mut held, "LET", &[bindings, result])?;
        held.get(expansion).copied().ok_or(ObjectError::Layout)
    })
}

#[allow(clippy::too_many_lines)]
fn expand_input(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = **roots.first().ok_or(ObjectError::TypeError)?;
        let spec_parts = elements(ctx, spec)?;
        let variable = spec_parts.first().copied().ok_or(ObjectError::TypeError)?;
        let string = spec_parts
            .get(1)
            .copied()
            .unwrap_or(ncl_object::make_string(ctx, runtime, &[])?);
        if !matches!(classify_object(ctx, variable), ObjectRef::Symbol(_)) {
            return Err(ObjectError::TypeError);
        }
        let mut start = Word::NIL;
        let mut end = Word::NIL;
        let mut has_start = false;
        let mut has_end = false;
        let mut index = Word::NIL;
        let mut has_index = false;
        let mut allow_other_keys = false;
        let mut unknown_keyword = false;
        let mut cursor = 2;
        while cursor < spec_parts.len() {
            let key = *spec_parts.get(cursor).ok_or(ObjectError::Layout)?;
            let value = *spec_parts.get(cursor + 1).ok_or(ObjectError::TypeError)?;
            if is_keyword(ctx, runtime, key, "START")? {
                if has_start {
                    return Err(ObjectError::TypeError);
                }
                has_start = true;
                start = value;
            } else if is_keyword(ctx, runtime, key, "END")? {
                if has_end {
                    return Err(ObjectError::TypeError);
                }
                has_end = true;
                end = value;
            } else if is_keyword(ctx, runtime, key, "ALLOW-OTHER-KEYS")? {
                allow_other_keys |= value != Word::NIL;
            } else if is_keyword(ctx, runtime, key, "INDEX")? {
                if has_index || !matches!(classify_object(ctx, value), ObjectRef::Symbol(_)) {
                    return Err(ObjectError::TypeError);
                }
                has_index = true;
                index = value;
            } else {
                if !is_keyword_symbol(ctx, runtime, key)? {
                    ctx.set_pending_lisp_error(LispError::ProgramError(
                        ProgramError::UnknownKeyword,
                    ));
                    return Err(ObjectError::TypeError);
                }
                unknown_keyword = true;
            }
            cursor += 2;
        }
        if unknown_keyword && !allow_other_keys {
            ctx.set_pending_lisp_error(LispError::ProgramError(ProgramError::UnknownKeyword));
            return Err(ObjectError::TypeError);
        }

        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let variable_index = held.len();
        held.push(variable);
        let string_index = held.len();
        held.push(string);
        let start_index = held.len();
        held.push(start);
        let end_index = held.len();
        held.push(end);
        let index_index = held.len();
        held.push(index);
        let index_value_index = held.len();
        held.push(Word::NIL);
        let mut stream_indexes = vec![string_index];
        if has_start || has_end {
            if !has_start {
                *held.get_mut(start_index).ok_or(ObjectError::Layout)? = Word::fixnum(0);
            }
            stream_indexes.push(start_index);
        }
        if has_end {
            stream_indexes.push(end_index);
        }
        let stream = held_form(
            ctx,
            runtime,
            &mut held,
            "MAKE-STRING-INPUT-STREAM",
            &stream_indexes,
        )?;
        let binding = held_list(ctx, runtime, &mut held, &[variable_index, stream])?;
        let bindings = held_list(ctx, runtime, &mut held, &[binding])?;
        let body = held_form(
            ctx,
            runtime,
            &mut held,
            "PROGN",
            &(1..roots.len()).collect::<Vec<_>>(),
        )?;
        let body = if has_index {
            let position = held_form(ctx, runtime, &mut held, "FILE-POSITION", &[stream])?;
            let update = held_form(ctx, runtime, &mut held, "SETQ", &[index_index, position])?;
            held_form(ctx, runtime, &mut held, "PROG1", &[body, update])?
        } else {
            body
        };
        let bindings = if has_index {
            let index_binding =
                held_list(ctx, runtime, &mut held, &[index_index, index_value_index])?;
            held_list(ctx, runtime, &mut held, &[binding, index_binding])?
        } else {
            bindings
        };
        let expansion = held_form(ctx, runtime, &mut held, "LET", &[bindings, body])?;
        held.get(expansion).copied().ok_or(ObjectError::Layout)
    })
}

pub fn expand_output_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.required(0)?;
    let mut arguments = elements(ctx, form)?;
    if arguments.is_empty() {
        return Err(ObjectError::TypeError);
    }
    arguments.remove(0);
    expand_output(ctx, runtime, &arguments)
}

pub fn expand_input_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.required(0)?;
    let mut arguments = elements(ctx, form)?;
    if arguments.is_empty() {
        return Err(ObjectError::TypeError);
    }
    arguments.remove(0);
    expand_input(ctx, runtime, &arguments)
}

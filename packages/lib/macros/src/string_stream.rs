//! Expanders for the string stream convenience macros.

use crate::{elements, list, symbol};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
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

fn is_keyword(ctx: &ThreadContext, word: Word, expected: &str) -> Result<bool> {
    let ObjectRef::Symbol(symbol) = classify_object(ctx, word) else {
        return Ok(false);
    };
    let name = ncl_object::symbol_name(ctx, symbol)?;
    let actual = (0..ncl_object::string_length(ctx, name)?)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect::<std::result::Result<String, _>>()?;
    Ok(actual == expected)
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

fn expand_input(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = **roots.first().ok_or(ObjectError::TypeError)?;
        let spec_parts = elements(ctx, spec)?;
        let variable = spec_parts.first().copied().ok_or(ObjectError::TypeError)?;
        let string = spec_parts.get(1).copied().ok_or(ObjectError::TypeError)?;
        if !matches!(classify_object(ctx, variable), ObjectRef::Symbol(_)) {
            return Err(ObjectError::TypeError);
        }
        let mut start = Word::NIL;
        let mut end = Word::NIL;
        let mut cursor = 2;
        while cursor < spec_parts.len() {
            let key = *spec_parts.get(cursor).ok_or(ObjectError::Layout)?;
            if is_keyword(ctx, key, "START")? {
                let value = *spec_parts.get(cursor + 1).ok_or(ObjectError::TypeError)?;
                if start != Word::NIL {
                    return Err(ObjectError::TypeError);
                }
                start = value;
            } else if is_keyword(ctx, key, "END")? {
                let value = *spec_parts.get(cursor + 1).ok_or(ObjectError::TypeError)?;
                if end != Word::NIL {
                    return Err(ObjectError::TypeError);
                }
                end = value;
            } else {
                break;
            }
            cursor += 2;
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
        let mut stream_indexes = vec![string_index];
        if start != Word::NIL || end != Word::NIL {
            if start == Word::NIL {
                *held.get_mut(start_index).ok_or(ObjectError::Layout)? = Word::fixnum(0);
            }
            stream_indexes.push(start_index);
        }
        if end != Word::NIL {
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

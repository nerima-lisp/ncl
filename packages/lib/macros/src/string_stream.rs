//! Expanders for the string stream convenience macros.

use crate::{elements, list, symbol};
use ncl_object::{
    BuiltinArgs, HeapRootedWord, HeapRootedWords, LispError, MultipleValues, ObjectError,
    ObjectRef, ProgramError, Runtime, ThreadContext, Word, classify_object, symbol_package,
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

fn quoted_symbol_name(ctx: &mut ThreadContext, value: Word) -> Result<Option<String>> {
    let Ok(parts) = elements(ctx, value) else {
        return Ok(None);
    };
    if parts.len() != 2 {
        return Ok(None);
    }
    let Some(operator) = parts
        .first()
        .copied()
        .map(|part| classify_object(ctx, part))
    else {
        return Ok(None);
    };
    let ObjectRef::Symbol(operator) = operator else {
        return Ok(None);
    };
    let operator_name = ncl_object::symbol_name(ctx, operator)?;
    let operator_name = (0..ncl_object::string_length(ctx, operator_name)?)
        .map(|index| ncl_object::string_ref(ctx, operator_name, index))
        .collect::<std::result::Result<String, _>>()?;
    if operator_name != "QUOTE" {
        return Ok(None);
    }
    let Some(symbol) = parts.get(1).copied().map(|part| classify_object(ctx, part)) else {
        return Ok(None);
    };
    let ObjectRef::Symbol(symbol) = symbol else {
        return Ok(None);
    };
    let name = ncl_object::symbol_name(ctx, symbol)?;
    Ok(Some(
        (0..ncl_object::string_length(ctx, name)?)
            .map(|index| ncl_object::string_ref(ctx, name, index))
            .collect::<std::result::Result<String, _>>()?,
    ))
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

fn rooted_held_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut HeapRootedWords,
    name: &str,
    indexes: &[usize],
) -> Result<usize> {
    let args = indexes
        .iter()
        .map(|index| {
            held.get(*index)
                .map(HeapRootedWord::get)
                .ok_or(ObjectError::TypeError)
        })
        .collect::<Result<Vec<_>>>()?;
    let value = form(ctx, runtime, name, &args)?;
    held.push(ctx, value)?;
    Ok(held.len() - 1)
}

fn rooted_held_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut HeapRootedWords,
    indexes: &[usize],
) -> Result<usize> {
    let values = indexes
        .iter()
        .map(|index| {
            held.get(*index)
                .map(HeapRootedWord::get)
                .ok_or(ObjectError::TypeError)
        })
        .collect::<Result<Vec<_>>>()?;
    let value = list(ctx, runtime, &values)?;
    held.push(ctx, value)?;
    Ok(held.len() - 1)
}

fn expand_output(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        let variable = spec.first().copied().ok_or(ObjectError::Layout)?;
        if !matches!(classify_object(ctx, variable), ObjectRef::Symbol(_)) {
            return Err(ObjectError::TypeError);
        }

        let mut initial_string = Word::NIL;
        let mut has_initial_string = false;
        let mut allow_other_keys = false;
        let mut unknown_keyword = false;
        let mut cursor = 1;
        if let Some(value) = spec.get(cursor).copied()
            && !is_keyword_symbol(ctx, runtime, value)?
        {
            initial_string = value;
            has_initial_string = true;
            cursor += 1;
        }
        while cursor < spec.len() {
            let key = *spec.get(cursor).ok_or(ObjectError::Layout)?;
            let value = *spec.get(cursor + 1).ok_or(ObjectError::TypeError)?;
            if is_keyword(ctx, runtime, key, "ELEMENT-TYPE")? {
                if !has_initial_string
                    && (value == Word::NIL
                        || quoted_symbol_name(ctx, value)?
                            .is_some_and(|name| name != "CHARACTER" && name != "T"))
                {
                    return Err(ObjectError::TypeError);
                }
            } else if is_keyword(ctx, runtime, key, "ALLOW-OTHER-KEYS")? {
                allow_other_keys |= value != Word::NIL;
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
        let body_end = held.len();
        let variable_index = held.len();
        held.push(variable);
        let initial_string_index = held.len();
        held.push(initial_string);
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
        let body = if has_initial_string {
            let write_initial = held_form(
                ctx,
                runtime,
                &mut held,
                "WRITE-STRING",
                &[initial_string_index, variable_index],
            )?;
            held_form(ctx, runtime, &mut held, "PROGN", &[write_initial, body])?
        } else {
            body
        };
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
fn expand_input(ctx: &mut ThreadContext, runtime: &Runtime, values: &[HeapRootedWord]) -> Result {
    let spec = values.first().ok_or(ObjectError::TypeError)?.get();
    let raw_spec_parts = elements(ctx, spec)?;
    let spec_parts = HeapRootedWords::new(ctx, &raw_spec_parts)?;
    let input_values = values.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
    let mut held = HeapRootedWords::new(ctx, &input_values)?;
    let variable_index = held.len();
    held.push(ctx, spec_parts.first().ok_or(ObjectError::TypeError)?.get())?;
    let string_index = held.len();
    let default_string = ncl_object::make_string(ctx, runtime, &[])?;
    let string = spec_parts
        .get(1)
        .map_or(default_string, HeapRootedWord::get);
    held.push(ctx, string)?;
    if !matches!(
        classify_object(
            ctx,
            held.get(variable_index).ok_or(ObjectError::Layout)?.get()
        ),
        ObjectRef::Symbol(_)
    ) {
        return Err(ObjectError::TypeError);
    }
    let start_index = held.len();
    held.push(ctx, Word::NIL)?;
    let end_index = held.len();
    held.push(ctx, Word::NIL)?;
    let index_index = held.len();
    held.push(ctx, Word::NIL)?;
    let index_value_index = held.len();
    held.push(ctx, Word::NIL)?;

    let mut has_start = false;
    let mut has_end = false;
    let mut has_index = false;
    let mut allow_other_keys = false;
    let mut unknown_keyword = false;
    let mut cursor = 2;
    while cursor < spec_parts.len() {
        let key = spec_parts.get(cursor).ok_or(ObjectError::Layout)?.get();
        let value = spec_parts
            .get(cursor + 1)
            .ok_or(ObjectError::TypeError)?
            .get();
        if is_keyword(ctx, runtime, key, "START")? {
            if has_start {
                return Err(ObjectError::TypeError);
            }
            has_start = true;
            held.get(start_index).ok_or(ObjectError::Layout)?.set(value);
        } else if is_keyword(ctx, runtime, key, "END")? {
            if has_end {
                return Err(ObjectError::TypeError);
            }
            has_end = true;
            held.get(end_index).ok_or(ObjectError::Layout)?.set(value);
        } else if is_keyword(ctx, runtime, key, "ALLOW-OTHER-KEYS")? {
            allow_other_keys |= value != Word::NIL;
        } else if is_keyword(ctx, runtime, key, "INDEX")? {
            if has_index || !matches!(classify_object(ctx, value), ObjectRef::Symbol(_)) {
                return Err(ObjectError::TypeError);
            }
            has_index = true;
            held.get(index_index).ok_or(ObjectError::Layout)?.set(value);
        } else {
            if !is_keyword_symbol(ctx, runtime, key)? {
                ctx.set_pending_lisp_error(LispError::ProgramError(ProgramError::UnknownKeyword));
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

    let mut stream_indexes = vec![string_index];
    if has_start || has_end {
        if !has_start {
            held.get(start_index)
                .ok_or(ObjectError::Layout)?
                .set(Word::fixnum(0));
        }
        stream_indexes.push(start_index);
    }
    if has_end {
        stream_indexes.push(end_index);
    }
    let stream = rooted_held_form(
        ctx,
        runtime,
        &mut held,
        "MAKE-STRING-INPUT-STREAM",
        &stream_indexes,
    )?;
    let binding = rooted_held_list(ctx, runtime, &mut held, &[variable_index, stream])?;
    let mut bindings = rooted_held_list(ctx, runtime, &mut held, &[binding])?;
    let body = rooted_held_form(
        ctx,
        runtime,
        &mut held,
        "PROGN",
        &(1..values.len()).collect::<Vec<_>>(),
    )?;
    let body = if has_index {
        let position = rooted_held_form(ctx, runtime, &mut held, "FILE-POSITION", &[stream])?;
        let update = rooted_held_form(ctx, runtime, &mut held, "SETQ", &[index_index, position])?;
        rooted_held_form(ctx, runtime, &mut held, "PROG1", &[body, update])?
    } else {
        body
    };
    if has_index {
        let index_binding =
            rooted_held_list(ctx, runtime, &mut held, &[index_index, index_value_index])?;
        bindings = rooted_held_list(ctx, runtime, &mut held, &[binding, index_binding])?;
    }
    let expansion = rooted_held_form(ctx, runtime, &mut held, "LET", &[bindings, body])?;
    held.get(expansion)
        .map(HeapRootedWord::get)
        .ok_or(ObjectError::Layout)
}

fn arguments(ctx: &mut ThreadContext, form: Word) -> Result<HeapRootedWords> {
    let mut values = elements(ctx, form)?;
    if values.is_empty() {
        return Err(ObjectError::TypeError);
    }
    values.remove(0);
    HeapRootedWords::new(ctx, &values)
}

pub fn expand_output_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.required(0)?;
    let arguments = arguments(ctx, form)?;
    let words = arguments
        .iter()
        .map(HeapRootedWord::get)
        .collect::<Vec<_>>();
    expand_output(ctx, runtime, &words)
}

pub fn expand_input_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.required(0)?;
    let arguments = arguments(ctx, form)?;
    expand_input(ctx, runtime, arguments.as_slice())
}

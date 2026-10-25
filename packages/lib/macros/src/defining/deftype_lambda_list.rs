use ncl_object::{ObjectError, ObjectRef, Runtime, ThreadContext, Word, classify_object};

use crate::form::{list, symbol};

use super::{form_elements, quote};

pub(super) fn deftype_lambda_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    lambda_list: Word,
) -> Result<Word, ObjectError> {
    let (parts, tail) = lambda_list_parts(ctx, lambda_list)?;
    ncl_object::with_roots(ctx, &parts, |ctx, roots| {
        let tail = ncl_object::HeapRootedWord::new(ctx, tail)?;
        let mut default_star = symbol(ctx, runtime, "*")?;
        ncl_object::with_root(ctx, &mut default_star, |ctx, default_star| {
            let default_form = quote(ctx, runtime, *default_star)?;
            ncl_object::with_root(ctx, &mut default_form.clone(), |ctx, default_form| {
                let mut section = None;
                let mut values = Vec::with_capacity(roots.len());
                for part in roots {
                    let word = part.get();
                    if symbol_named(ctx, word, "&OPTIONAL")? {
                        section = Some(false);
                        values.push(word);
                    } else if symbol_named(ctx, word, "&KEY")? {
                        section = Some(true);
                        values.push(word);
                    } else if is_lambda_list_marker(ctx, word)? {
                        section = None;
                        values.push(word);
                    } else if let Some(is_key) = section {
                        values.push(defaulted_parameter(
                            ctx,
                            runtime,
                            word,
                            default_form.get(),
                            is_key,
                        )?);
                    } else if word.is_cons() {
                        values.push(deftype_lambda_list(ctx, runtime, word)?);
                    } else {
                        values.push(word);
                    }
                }
                list_with_tail(ctx, runtime, &values, tail.get())
            })
        })
    })
}

fn lambda_list_parts(
    ctx: &ThreadContext,
    mut lambda_list: Word,
) -> Result<(Vec<Word>, Word), ObjectError> {
    let mut parts = Vec::new();
    while lambda_list.is_cons() {
        parts.push(ncl_object::car(ctx, lambda_list)?);
        lambda_list = ncl_object::cdr(ctx, lambda_list)?;
    }
    Ok((parts, lambda_list))
}

fn list_with_tail(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    tail: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, values| {
        let mut result = tail;
        for value in values.iter().rev() {
            result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
                ncl_object::make_cons(ctx, runtime, **value, *result)
            })?;
        }
        Ok(result)
    })
}

fn defaulted_parameter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    parameter: Word,
    default: Word,
    is_key: bool,
) -> Result<Word, ObjectError> {
    if !parameter.is_cons() {
        return list(ctx, runtime, &[parameter, default]);
    }
    let mut parts = form_elements(ctx, parameter)?;
    let Some(name) = parts.first().copied() else {
        return Ok(parameter);
    };
    let transformed_name = if is_key && name.is_cons() {
        name
    } else if name.is_cons() {
        deftype_lambda_list(ctx, runtime, name)?
    } else {
        name
    };
    if transformed_name != name {
        let first = parts.first_mut().ok_or(ObjectError::TypeError)?;
        *first = transformed_name;
    }
    if parts.len() == 1 {
        parts.push(default);
    } else if transformed_name == name {
        return Ok(parameter);
    }
    list(ctx, runtime, &parts)
}

fn is_lambda_list_marker(ctx: &ThreadContext, word: Word) -> Result<bool, ObjectError> {
    [
        "&REST",
        "&BODY",
        "&AUX",
        "&ALLOW-OTHER-KEYS",
        "&WHOLE",
        "&ENVIRONMENT",
        "&DESTRUCTURING",
    ]
    .iter()
    .try_fold(false, |found, name| {
        Ok(found || symbol_named(ctx, word, name)?)
    })
}

fn symbol_named(ctx: &ThreadContext, word: Word, name: &str) -> Result<bool, ObjectError> {
    if !matches!(classify_object(ctx, word), ObjectRef::Symbol(_)) {
        return Ok(false);
    }
    let name_word = ncl_object::symbol_name(ctx, word)?;
    let expected = name.chars().collect::<Vec<_>>();
    let length = ncl_object::string_length(ctx, name_word)?;
    if length != expected.len() {
        return Ok(false);
    }
    for (index, character) in expected.iter().enumerate() {
        if ncl_object::string_ref(ctx, name_word, index)? != *character {
            return Ok(false);
        }
    }
    Ok(true)
}

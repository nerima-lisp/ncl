//! Expanders for file and existing-stream convenience macros.

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
                    .ok_or(ObjectError::Layout)
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
                    .ok_or(ObjectError::Layout)
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

fn expand(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word], open: bool) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        let variable = *spec.first().ok_or(ObjectError::TypeError)?;
        if !matches!(classify_object(ctx, variable), ObjectRef::Symbol(_)) {
            return Err(ObjectError::TypeError);
        }
        let resource = if open {
            let filespec = *spec.get(1).ok_or(ObjectError::TypeError)?;
            let mut args = vec![filespec];
            args.extend(spec.get(2..).unwrap_or(&[]));
            form(ctx, runtime, "OPEN", &args)?
        } else {
            *spec.get(1).ok_or(ObjectError::TypeError)?
        };

        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let variable_index = held.len();
        held.push(variable);
        let resource_index = held.len();
        held.push(resource);
        let binding = held_list(ctx, runtime, &mut held, &[variable_index, resource_index])?;
        let bindings = held_list(ctx, runtime, &mut held, &[binding])?;
        let body = held_form(
            ctx,
            runtime,
            &mut held,
            "PROGN",
            &(1..roots.len()).collect::<Vec<_>>(),
        )?;
        let close = held_form(ctx, runtime, &mut held, "CLOSE", &[variable_index])?;
        let cleanup = held_form(ctx, runtime, &mut held, "WHEN", &[variable_index, close])?;
        let protected = held_form(ctx, runtime, &mut held, "PROG1", &[body, cleanup])?;
        let expansion = held_form(ctx, runtime, &mut held, "LET", &[bindings, protected])?;
        held.get(expansion).copied().ok_or(ObjectError::Layout)
    })
}

pub fn expand_file_adapter(
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
    expand(ctx, runtime, &arguments, true)
}

pub fn expand_stream_adapter(
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
    expand(ctx, runtime, &arguments, false)
}

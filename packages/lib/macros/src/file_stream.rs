//! Expander for the file stream convenience macro.

use crate::{elements, fresh_symbol, list, symbol};
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

fn held_symbol(ctx: &mut ThreadContext, runtime: &Runtime, held: &mut Vec<Word>) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = fresh_symbol(ctx, runtime)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
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

fn expand(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        if spec.len() < 2 || (spec.len() - 2) % 2 != 0 {
            return Err(ObjectError::TypeError);
        }
        let variable = *spec.first().ok_or(ObjectError::TypeError)?;
        if !matches!(classify_object(ctx, variable), ObjectRef::Symbol(_)) {
            return Err(ObjectError::TypeError);
        }

        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let variable_index = held.len();
        held.push(variable);
        let filespec_index = held.len();
        held.push(spec.get(1).copied().ok_or(ObjectError::TypeError)?);
        let mut open_indexes = vec![filespec_index];
        for option in spec.iter().skip(2) {
            let option_index = held.len();
            held.push(*option);
            open_indexes.push(option_index);
        }
        let stream_index = held_symbol(ctx, runtime, &mut held)?;
        let abort_index = held_symbol(ctx, runtime, &mut held)?;
        let true_index = held_named_symbol(ctx, runtime, &mut held, "T")?;

        let open_call = held_form(ctx, runtime, &mut held, "OPEN", &open_indexes)?;
        let stream_binding = held_list(ctx, runtime, &mut held, &[stream_index, open_call])?;
        let abort_binding = held_list(ctx, runtime, &mut held, &[abort_index, true_index])?;
        let outer_bindings = held_list(ctx, runtime, &mut held, &[stream_binding, abort_binding])?;
        let user_stream_binding =
            held_list(ctx, runtime, &mut held, &[variable_index, stream_index])?;
        let user_stream_bindings = held_list(ctx, runtime, &mut held, &[user_stream_binding])?;

        let declarations_symbol = held_named_symbol(ctx, runtime, &mut held, "DECLARE")?;
        let mut body_start = 1;
        let mut declarations = Vec::new();
        while let Some(body_form) = roots.get(body_start).map(|root| **root) {
            if !body_form.is_cons()
                || ncl_object::car(ctx, body_form)?
                    != held
                        .get(declarations_symbol)
                        .copied()
                        .ok_or(ObjectError::Layout)?
            {
                break;
            }
            declarations.push(body_start);
            body_start += 1;
        }

        let body_indexes = (body_start..roots.len()).collect::<Vec<_>>();
        let body = held_form(ctx, runtime, &mut held, "PROGN", &body_indexes)?;
        let nil_index = held.len();
        held.push(Word::NIL);
        let mark_normal_exit =
            held_form(ctx, runtime, &mut held, "SETQ", &[abort_index, nil_index])?;
        let protected_body = held_form(
            ctx,
            runtime,
            &mut held,
            "MULTIPLE-VALUE-PROG1",
            &[body, mark_normal_exit],
        )?;

        let abort_keyword = held_named_symbol(ctx, runtime, &mut held, "KEYWORD::ABORT")?;
        let close = held_form(
            ctx,
            runtime,
            &mut held,
            "CLOSE",
            &[stream_index, abort_keyword, abort_index],
        )?;
        let close_if_open = held_form(ctx, runtime, &mut held, "IF", &[stream_index, close])?;
        let unwind = held_form(
            ctx,
            runtime,
            &mut held,
            "UNWIND-PROTECT",
            &[protected_body, close_if_open],
        )?;

        let let_symbol = held_named_symbol(ctx, runtime, &mut held, "LET")?;
        let mut inner_let_indexes = vec![let_symbol, user_stream_bindings];
        inner_let_indexes.extend(declarations);
        inner_let_indexes.push(unwind);
        let inner_let = held_list(ctx, runtime, &mut held, &inner_let_indexes)?;
        let outer_let = held_form(
            ctx,
            runtime,
            &mut held,
            "LET*",
            &[outer_bindings, inner_let],
        )?;
        held.get(outer_let).copied().ok_or(ObjectError::Layout)
    })
}

pub fn expand_with_open_file_adapter(
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
    expand(ctx, runtime, &arguments)
}

use super::progn;
use super::{args, form, symbol};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub(crate) fn expand_multiple_value_setq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    if values.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    let variables = crate::elements(ctx, *values.first().ok_or(ObjectError::TypeError)?)?;
    let value = *values.get(1).ok_or(ObjectError::TypeError)?;
    if variables
        .iter()
        .any(|variable| !matches!(classify_object(ctx, *variable), ObjectRef::Symbol(_)))
    {
        return Err(ObjectError::TypeError);
    }
    let mut temporaries = Vec::with_capacity(variables.len());
    for _ in &variables {
        temporaries.push(crate::fresh_symbol(ctx, runtime)?);
    }
    let temporary_list = crate::list(ctx, runtime, &temporaries)?;
    let mut body = Vec::with_capacity(variables.len() + 1);
    for (variable, temporary) in variables.iter().zip(temporaries.iter()) {
        body.push(form(ctx, runtime, "SETQ", &[*variable, *temporary])?);
    }
    body.push(temporaries.first().copied().unwrap_or(Word::NIL));
    let body = progn(ctx, runtime, &body)?;
    form(
        ctx,
        runtime,
        "MULTIPLE-VALUE-BIND",
        &[temporary_list, value, body],
    )
}

pub(crate) fn expand_multiple_value_list_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let words = (0..input.len())
        .filter_map(|index| input.get(index))
        .collect::<Vec<_>>();
    let arguments = args(ctx, words.first().copied().ok_or(ObjectError::TypeError)?)?;
    ncl_object::with_roots(ctx, &arguments, |ctx, roots| {
        // check-added-lines: allow(index) exact-shape destructuring
        let value = roots.first().ok_or(ObjectError::TypeError)?;
        let mut list_symbol = symbol(ctx, runtime, "LIST")?;
        ncl_object::with_root(ctx, &mut list_symbol, |ctx, list_symbol| {
            let mut list_function = form(ctx, runtime, "FUNCTION", &[*list_symbol])?;
            ncl_object::with_root(ctx, &mut list_function, |ctx, list_function| {
                form(
                    ctx,
                    runtime,
                    "MULTIPLE-VALUE-CALL",
                    &[*list_function, **value],
                )
            })
        })
    })
}

pub(crate) fn expand_multiple_value_bind_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let words = (0..input.len())
        .filter_map(|index| input.get(index))
        .collect::<Vec<_>>();
    let arguments = args(ctx, words.first().copied().ok_or(ObjectError::TypeError)?)?;
    super::multiple_value_bind::expand(ctx, runtime, &arguments)
}

//! Expansion of `WITH-OPEN-FILE`.

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

pub fn expand_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let words = (0..input.len())
        .filter_map(|index| input.get(index))
        .collect::<Vec<_>>();
    let first = *words.first().ok_or(ObjectError::TypeError)?;
    let first_parts = elements(ctx, first)?;
    let full_form = first_parts
        .first()
        .is_some_and(|head| symbol(ctx, runtime, "WITH-OPEN-FILE").is_ok_and(|name| *head == name));
    let arguments = if full_form {
        first_parts.get(1..).ok_or(ObjectError::TypeError)?.to_vec()
    } else {
        words
    };
    ncl_object::with_roots(ctx, &arguments, |ctx, roots| {
        let spec = elements(ctx, **roots.first().ok_or(ObjectError::TypeError)?)?;
        let variable = *spec.first().ok_or(ObjectError::TypeError)?;
        if !matches!(classify_object(ctx, variable), ObjectRef::Symbol(_)) {
            return Err(ObjectError::TypeError);
        }
        if spec.get(1).is_none() {
            return Err(ObjectError::TypeError);
        }
        let body = roots.get(1..).ok_or(ObjectError::TypeError)?;
        ncl_object::with_roots(ctx, &spec, |ctx, spec_roots| {
            let open_args = spec_roots
                .get(1..)
                .ok_or(ObjectError::TypeError)?
                .iter()
                .map(|value| **value)
                .collect::<Vec<_>>();
            let mut open = form(ctx, runtime, "OPEN", &open_args)?;
            ncl_object::with_root(ctx, &mut open, |ctx, open| {
                let mut binding = list(ctx, runtime, &[variable, *open])?;
                ncl_object::with_root(ctx, &mut binding, |ctx, binding| {
                    let mut bindings = list(ctx, runtime, &[*binding])?;
                    ncl_object::with_root(ctx, &mut bindings, |ctx, bindings| {
                        let body_words = body.iter().map(|value| **value).collect::<Vec<_>>();
                        let mut protected = form(ctx, runtime, "PROGN", &body_words)?;
                        ncl_object::with_root(ctx, &mut protected, |ctx, protected| {
                            let mut close = form(ctx, runtime, "CLOSE", &[variable])?;
                            ncl_object::with_root(ctx, &mut close, |ctx, close| {
                                let mut cleanup = form(ctx, runtime, "WHEN", &[variable, *close])?;
                                ncl_object::with_root(ctx, &mut cleanup, |ctx, cleanup| {
                                    let mut unwind = form(
                                        ctx,
                                        runtime,
                                        "UNWIND-PROTECT",
                                        &[*protected, *cleanup],
                                    )?;
                                    ncl_object::with_root(ctx, &mut unwind, |ctx, unwind| {
                                        form(ctx, runtime, "LET", &[*bindings, *unwind])
                                    })
                                })
                            })
                        })
                    })
                })
            })
        })
    })
}

use super::{args, form, symbol};
use ncl_object::{BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

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

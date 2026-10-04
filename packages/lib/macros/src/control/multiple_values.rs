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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapters_reject_missing_forms_and_build_list_call() -> Result<()> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let mut values = MultipleValues::new();
        let empty = BuiltinArgs::new(&[]);
        assert_eq!(
            expand_multiple_value_list_adapter(&mut ctx, &runtime, &empty, &mut values),
            Err(ObjectError::TypeError)
        );
        let x = super::super::symbol(&mut ctx, &runtime, "X")?;
        let operator = super::super::symbol(&mut ctx, &runtime, "MULTIPLE-VALUE-LIST")?;
        let form = super::super::form(&mut ctx, &runtime, "MULTIPLE-VALUE-LIST", &[x])?;
        let forms = [form];
        let args = BuiltinArgs::new(&forms);
        let expanded = expand_multiple_value_list_adapter(&mut ctx, &runtime, &args, &mut values)?;
        assert_eq!(
            super::super::elements(&mut ctx, expanded)?[0],
            super::super::symbol(&mut ctx, &runtime, "MULTIPLE-VALUE-CALL")?
        );
        assert_ne!(operator, Word::NIL);
        Ok(())
    }
}

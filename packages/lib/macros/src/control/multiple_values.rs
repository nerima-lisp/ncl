use super::{args, form, symbol};
use ncl_object::{
    BuiltinArgs, HeapRootedWord, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
};

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
    // check-added-lines: allow(index) exact-shape destructuring
    let value = arguments.first().ok_or(ObjectError::TypeError)?;
    let list_symbol = symbol(ctx, runtime, "LIST")?;
    let list_symbol = HeapRootedWord::new(ctx, list_symbol)?;
    let list_function = form(ctx, runtime, "FUNCTION", &[list_symbol.get()])?;
    let list_function = HeapRootedWord::new(ctx, list_function)?;
    form(
        ctx,
        runtime,
        "MULTIPLE-VALUE-CALL",
        &[list_function.get(), value.get()],
    )
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
    super::multiple_value_bind::expand(ctx, runtime, arguments.as_slice())
}

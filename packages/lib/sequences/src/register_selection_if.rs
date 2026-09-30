//! `FIND-IF`/`POSITION-IF`/`COUNT-IF` (and `-IF-NOT`) builtin entries, split
//! out of `register.rs` to keep that file under the project's line-count
//! limit.

use super::{
    BuiltinArgs, BuiltinFunctionCaller, Local, MultipleValues, ObjectError, Runtime, Scope,
    ThreadContext, Word, domain,
};

type SelectionIfOperation = fn(
    &mut ThreadContext,
    &Runtime,
    &mut BuiltinFunctionCaller,
    &mut [Word], // check-added-lines: allow(index) slice type
    Word,
    domain::selection::SelectionOptions,
    bool,
) -> Result<Word, ObjectError>;

fn selection_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    operation: SelectionIfOperation,
    negate: bool,
) -> Result<Word, ObjectError> {
    let (positional, options) = domain::selection::parse_options(ctx, args.as_slice())?;
    let sequence = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let mut scope = Scope::new(ctx);
    let sequence_handle = scope.root::<Word>(Local::from_word(sequence));
    let result = (|| {
        let sequence = scope.get(sequence_handle).as_word();
        let sequence_value = domain::selection::object_sequence(scope.context(), sequence)?;
        let mut items = domain::selection::sequence_values(scope.context_mut(), sequence_value)?;
        let predicate = *positional.first().ok_or(ObjectError::TypeError)?;
        let mut caller = BuiltinFunctionCaller;
        operation(
            scope.context_mut(),
            runtime,
            &mut caller,
            &mut items,
            predicate,
            options,
            negate,
        )
    })();
    let result = result?;
    let result_handle = scope.root::<Word>(Local::from_word(result));
    values.clear();
    Ok(scope.get(result_handle).as_word())
}

pub fn find_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_if_entry(
        ctx,
        runtime,
        args,
        values,
        domain::selection::find_if,
        false,
    )
}
pub fn find_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_if_entry(ctx, runtime, args, values, domain::selection::find_if, true)
}
pub fn position_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_if_entry(
        ctx,
        runtime,
        args,
        values,
        domain::selection::position_if,
        false,
    )
}
pub fn position_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_if_entry(
        ctx,
        runtime,
        args,
        values,
        domain::selection::position_if,
        true,
    )
}
pub fn count_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_if_entry(
        ctx,
        runtime,
        args,
        values,
        domain::selection::count_if,
        false,
    )
}
pub fn count_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_if_entry(
        ctx,
        runtime,
        args,
        values,
        domain::selection::count_if,
        true,
    )
}

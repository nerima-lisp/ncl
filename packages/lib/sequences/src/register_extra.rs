use super::{
    BuiltinArgs, BuiltinFunctionCaller, MultipleValues, ObjectError, Runtime, Sequence,
    ThreadContext, Word, callback_word, domain, selection_parse, sequence_arg,
};

pub(super) fn selection_transform_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    replacement: Option<Word>,
) -> Result<Word, ObjectError> {
    let (positional, options) = if replacement.is_some() {
        selection_parse(ctx, args.as_slice().get(1..).ok_or(ObjectError::TypeError)?)?
    } else {
        selection_parse(ctx, args.as_slice())?
    };
    let object = *positional.first().ok_or(ObjectError::TypeError)?;
    let sequence = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let mut items = domain::selection::sequence_values(
        ctx,
        domain::selection::object_sequence(ctx, sequence)?,
    )?;
    let mut caller = BuiltinFunctionCaller;
    let result = if let Some(replacement) = replacement {
        domain::selection::substitute(
            ctx,
            runtime,
            &mut caller,
            &mut items,
            replacement,
            object,
            options,
        )?
    } else {
        domain::selection::remove(ctx, runtime, &mut caller, &mut items, object, options)?
    };
    values.clear();
    let mut result = result;
    domain::selection::list_from_values(ctx, runtime, &mut result)
}
pub(super) fn remove_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_entry(ctx, runtime, args, values, None)
}
pub(super) fn substitute_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let replacement = args.required(0)?;
    selection_transform_entry(ctx, runtime, args, values, Some(replacement))
}
pub(super) fn hof_sequences(
    ctx: &ThreadContext,
    words: &[Word],
) -> Result<Vec<Sequence>, ObjectError> {
    words
        .iter()
        .map(|&word| domain::selection::object_sequence(ctx, word))
        .collect()
}
pub(super) fn mapcar_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let function = args.required(0)?;
    callback_word(ctx, function)?;
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::mapcar(
        ctx,
        runtime,
        function,
        args.as_slice().get(1..).ok_or(ObjectError::TypeError)?,
        &mut caller,
    );
    values.clear();
    result
}
pub(super) fn mapc_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let function = args.required(0)?;
    callback_word(ctx, function)?;
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::mapc(
        ctx,
        runtime,
        function,
        args.as_slice().get(1..).ok_or(ObjectError::TypeError)?,
        &mut caller,
    );
    values.clear();
    result
}
pub(super) fn map_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let destination = args.required(0)?;
    let function = args.required(1)?;
    callback_word(ctx, function)?;
    let sequences = hof_sequences(ctx, args.as_slice().get(2..).ok_or(ObjectError::TypeError)?)?;
    let result_type = if destination == Word::NIL {
        Sequence::List(ncl_object::List::Nil)
    } else {
        domain::selection::object_sequence(ctx, destination)?
    };
    let mut caller = BuiltinFunctionCaller;
    let result =
        domain::higher_order::map(ctx, runtime, result_type, function, &sequences, &mut caller);
    values.clear();
    result
}
pub(super) fn predicate_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    kind: domain::higher_order::Predicate,
) -> Result<Word, ObjectError> {
    let function = args.required(0)?;
    callback_word(ctx, function)?;
    let sequences = hof_sequences(ctx, args.as_slice().get(1..).ok_or(ObjectError::TypeError)?)?;
    let mut caller = BuiltinFunctionCaller;
    let result =
        domain::higher_order::predicate(ctx, runtime, kind, function, &sequences, &mut caller);
    values.clear();
    result
}
pub(super) fn every_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    predicate_entry(
        ctx,
        runtime,
        args,
        values,
        domain::higher_order::Predicate::Every,
    )
}
pub(super) fn some_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    predicate_entry(
        ctx,
        runtime,
        args,
        values,
        domain::higher_order::Predicate::Some,
    )
}
pub(super) fn notany_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    predicate_entry(
        ctx,
        runtime,
        args,
        values,
        domain::higher_order::Predicate::NotAny,
    )
}
pub(super) fn notevery_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    predicate_entry(
        ctx,
        runtime,
        args,
        values,
        domain::higher_order::Predicate::NotEvery,
    )
}
type ListMapOperation = fn(
    &mut ThreadContext,
    &Runtime,
    Word,
    &[Word],
    &mut BuiltinFunctionCaller,
) -> Result<Word, ObjectError>;
pub(super) fn list_map_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    operation: ListMapOperation,
) -> Result<Word, ObjectError> {
    let function = args.required(0)?;
    callback_word(ctx, function)?;
    let mut caller = BuiltinFunctionCaller;
    let result = operation(
        ctx,
        runtime,
        function,
        args.as_slice().get(1..).ok_or(ObjectError::TypeError)?,
        &mut caller,
    );
    values.clear();
    result
}
pub(super) fn map_into_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let destination = args.required(0)?;
    let function = args.required(1)?;
    callback_word(ctx, function)?;
    let sequences = hof_sequences(ctx, args.as_slice().get(2..).ok_or(ObjectError::TypeError)?)?;
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::map_into(
        ctx,
        runtime,
        destination,
        function,
        &sequences,
        &mut caller,
    );
    values.clear();
    result
}
pub(super) fn reduce_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let function = args.required(0)?;
    let sequence = sequence_arg(ctx, args.required(1)?)?;
    callback_word(ctx, function)?;
    let initial = args.get(2);
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::reduce(
        ctx,
        runtime,
        function,
        sequence,
        initial,
        false,
        &mut caller,
    );
    values.clear();
    result
}
type OrderSetOperation = fn(
    &mut ThreadContext,
    &Runtime,
    Word,
    Word,
    domain::order_sets::Options,
) -> Result<Word, ObjectError>;
pub(super) fn order_set_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    operation: OrderSetOperation,
) -> Result<Word, ObjectError> {
    let (positional, options) = domain::order_sets::parse_options(ctx, args.as_slice(), 2)?;
    let first = *positional.first().ok_or(ObjectError::TypeError)?;
    let second = *positional.get(1).ok_or(ObjectError::TypeError)?;
    values.clear();
    operation(ctx, runtime, first, second, options)
}
pub(super) fn set_sort_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    stable: bool,
) -> Result<Word, ObjectError> {
    let (positional, options) = domain::order_sets::parse_options(ctx, args.as_slice(), 2)?;
    let sequence = *positional.first().ok_or(ObjectError::TypeError)?;
    let predicate = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let result = domain::order_sets::sort(ctx, runtime, sequence, predicate, options.key, stable);
    values.clear();
    result
}
pub(super) fn sort_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    set_sort_entry(ctx, runtime, args, values, false)
}
pub(super) fn stable_sort_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    set_sort_entry(ctx, runtime, args, values, true)
}
pub(super) fn union_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::union)
}
pub(super) fn intersection_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::intersection)
}
pub(super) fn set_difference_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(
        ctx,
        runtime,
        args,
        values,
        domain::order_sets::set_difference,
    )
}
pub(super) fn set_exclusive_or_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(
        ctx,
        runtime,
        args,
        values,
        domain::order_sets::set_exclusive_or,
    )
}
pub(super) fn subsetp_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::subsetp)
}
pub(super) fn adjoin_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::adjoin)
}
pub(super) fn assoc_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::assoc)
}
pub(super) fn rassoc_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::rassoc)
}
pub(super) fn member_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::member)
}
pub(super) fn maplist_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_entry(ctx, runtime, args, values, domain::higher_order::maplist)
}
pub(super) fn mapl_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_entry(ctx, runtime, args, values, domain::higher_order::mapl)
}
pub(super) fn mapcan_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_entry(ctx, runtime, args, values, domain::higher_order::mapcan)
}
pub(super) fn mapcon_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_entry(ctx, runtime, args, values, domain::higher_order::mapcon)
}

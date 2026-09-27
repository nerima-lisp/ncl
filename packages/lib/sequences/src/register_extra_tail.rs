use super::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word, domain, list_map_entry,
    order_set_entry,
};

pub fn set_difference_entry(
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
pub fn set_exclusive_or_entry(
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
pub fn subsetp_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::subsetp)
}
pub fn adjoin_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::adjoin)
}
pub fn assoc_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::assoc)
}
pub fn rassoc_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::rassoc)
}
pub fn member_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    order_set_entry(ctx, runtime, args, values, domain::order_sets::member)
}
pub fn maplist_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_entry(ctx, runtime, args, values, domain::higher_order::maplist)
}
pub fn mapl_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_entry(ctx, runtime, args, values, domain::higher_order::mapl)
}
pub fn mapcan_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_entry(ctx, runtime, args, values, domain::higher_order::mapcan)
}
pub fn mapcon_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    list_map_entry(ctx, runtime, args, values, domain::higher_order::mapcon)
}

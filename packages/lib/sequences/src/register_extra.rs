use super::{
    BuiltinArgs, BuiltinFunctionCaller, MultipleValues, ObjectError, Runtime, Sequence,
    ThreadContext, Word, callback_word, domain, selection_parse, sequence_arg,
};
use ncl_object::{
    Handle, HandleVec, Local, ObjectRef, Scope, classify_object, string_length, string_ref,
    symbol_name,
};
mod register_extra_tail;
pub use register_extra_tail::{
    adjoin_entry, assoc_entry, mapcan_entry, mapcon_entry, mapl_entry, maplist_entry, member_entry,
    rassoc_entry, set_difference_entry, set_exclusive_or_entry, subsetp_entry,
};

fn root_args<'ctx>(
    scope: &mut Scope<'ctx>,
    args: &BuiltinArgs<'_>,
) -> Result<HandleVec<'ctx>, ObjectError> {
    let locals = (0..args.len())
        .map(|index| {
            args.get(index)
                .map(Local::from_word)
                .ok_or(ObjectError::Layout)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(scope.root_many(&locals))
}

fn words<'scope>(scope: &Scope<'scope>, handles: &HandleVec<'scope>) -> Vec<Word> {
    handles
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect()
}

fn word_at<'scope>(
    scope: &Scope<'scope>,
    handles: &HandleVec<'scope>,
    index: usize,
) -> Result<Word, ObjectError> {
    handles
        .as_slice()
        .get(index)
        .map(|handle| scope.get(*handle).as_word())
        .ok_or(ObjectError::TypeError)
}

fn keep_result<'scope>(
    scope: &mut Scope<'scope>,
    result: Result<Word, ObjectError>,
) -> Result<Word, ObjectError> {
    let handle: Handle<'scope, Word> = scope.root(Local::from_word(result?));
    Ok(scope.get(handle).as_word())
}

pub(super) fn selection_transform_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    replacement: Option<Word>,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    if replacement.is_some() {
        let rooted = words(&scope, &handles);
        let (positional, options) = selection_parse(
            scope.context_mut(),
            rooted.get(1..).ok_or(ObjectError::TypeError)?,
        )?;
        let object = *positional.first().ok_or(ObjectError::TypeError)?;
        let sequence = *positional.get(1).ok_or(ObjectError::TypeError)?;
        let sequence_type = domain::selection::object_sequence(scope.context(), sequence)?;
        let mut items = domain::selection::sequence_values(scope.context_mut(), sequence_type)?;
        let mut caller = BuiltinFunctionCaller;
        let indices = domain::selection::matching_indices(
            scope.context_mut(),
            runtime,
            &mut caller,
            &mut items,
            object,
            options,
        )?;
        let replaced = indices
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let replacement = word_at(&scope, &handles, 0)?;
        let mut result = Vec::with_capacity(items.len());
        for (index, value) in items.iter().enumerate() {
            result.push(if replaced.contains(&index) {
                replacement
            } else {
                *value
            });
        }
        values.clear();
        let result = domain::selection::list_from_values(scope.context_mut(), runtime, &mut result);
        return keep_result(&mut scope, result);
    }
    let rooted = words(&scope, &handles);
    let (positional, options) = selection_parse(scope.context_mut(), &rooted)?;
    let object = *positional.first().ok_or(ObjectError::TypeError)?;
    let sequence = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let sequence_type = domain::selection::object_sequence(scope.context(), sequence)?;
    let mut items = domain::selection::sequence_values(scope.context_mut(), sequence_type)?;
    let mut caller = BuiltinFunctionCaller;
    let result = domain::selection::remove(
        scope.context_mut(),
        runtime,
        &mut caller,
        &mut items,
        object,
        options,
    )?;
    values.clear();
    let mut result = result;
    let result = domain::selection::list_from_values(scope.context_mut(), runtime, &mut result);
    keep_result(&mut scope, result)
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
/// Shared implementation for `REMOVE-IF`/`REMOVE-IF-NOT` (`has_replacement`
/// false) and `SUBSTITUTE-IF`/`SUBSTITUTE-IF-NOT` (`has_replacement` true).
/// `negate` selects the `-IF-NOT` sense.
pub(super) fn selection_transform_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    has_replacement: bool,
    negate: bool,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let rooted = words(&scope, &handles);
    let (positional, options) = if has_replacement {
        selection_parse(
            scope.context_mut(),
            rooted.get(1..).ok_or(ObjectError::TypeError)?,
        )?
    } else {
        selection_parse(scope.context_mut(), &rooted)?
    };
    let predicate = *positional.first().ok_or(ObjectError::TypeError)?;
    let sequence = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let sequence_type = domain::selection::object_sequence(scope.context(), sequence)?;
    let mut items = domain::selection::sequence_values(scope.context_mut(), sequence_type)?;
    let mut caller = BuiltinFunctionCaller;
    let mut result = if has_replacement {
        let replacement = word_at(&scope, &handles, 0)?;
        domain::selection::substitute_if(
            scope.context_mut(),
            runtime,
            &mut caller,
            &mut items,
            replacement,
            predicate,
            options,
            negate,
        )?
    } else {
        domain::selection::remove_if(
            scope.context_mut(),
            runtime,
            &mut caller,
            &mut items,
            predicate,
            options,
            negate,
        )?
    };
    values.clear();
    let result = domain::selection::list_from_values(scope.context_mut(), runtime, &mut result);
    keep_result(&mut scope, result)
}
pub(super) fn remove_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_if_entry(ctx, runtime, args, values, false, false)
}
pub(super) fn remove_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_if_entry(ctx, runtime, args, values, false, true)
}
pub(super) fn substitute_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_if_entry(ctx, runtime, args, values, true, false)
}
pub(super) fn substitute_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_if_entry(ctx, runtime, args, values, true, true)
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
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let function_handle = handles
        .as_slice()
        .first()
        .copied()
        .ok_or(ObjectError::TypeError)?;
    let function = scope.get(function_handle).as_word();
    callback_word(scope.context(), function)?;
    let rooted = words(&scope, &handles);
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::mapcar(
        scope.context_mut(),
        runtime,
        function,
        rooted.get(1..).ok_or(ObjectError::TypeError)?,
        &mut caller,
    );
    values.clear();
    keep_result(&mut scope, result)
}
pub(super) fn mapc_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let function_handle = handles
        .as_slice()
        .first()
        .copied()
        .ok_or(ObjectError::TypeError)?;
    let function = scope.get(function_handle).as_word();
    callback_word(scope.context(), function)?;
    let rooted = words(&scope, &handles);
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::mapc(
        scope.context_mut(),
        runtime,
        function,
        rooted.get(1..).ok_or(ObjectError::TypeError)?,
        &mut caller,
    );
    values.clear();
    keep_result(&mut scope, result)
}
pub(super) fn map_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let destination = word_at(&scope, &handles, 0)?;
    let function = word_at(&scope, &handles, 1)?;
    callback_word(scope.context(), function)?;
    let rooted = words(&scope, &handles);
    let sequences = hof_sequences(
        scope.context(),
        rooted.get(2..).ok_or(ObjectError::TypeError)?,
    )?;
    let result_type = if destination == Word::NIL {
        Sequence::List(ncl_object::List::Nil)
    } else {
        domain::selection::object_sequence(scope.context(), destination)?
    };
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::map(
        scope.context_mut(),
        runtime,
        result_type,
        function,
        &sequences,
        &mut caller,
    );
    values.clear();
    if destination == Word::NIL {
        result?;
        return Ok(Word::NIL);
    }
    keep_result(&mut scope, result)
}
pub(super) fn predicate_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    kind: domain::higher_order::Predicate,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let function = word_at(&scope, &handles, 0)?;
    callback_word(scope.context(), function)?;
    let rooted = words(&scope, &handles);
    let sequences = hof_sequences(
        scope.context(),
        rooted.get(1..).ok_or(ObjectError::TypeError)?,
    )?;
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::predicate(
        scope.context_mut(),
        runtime,
        kind,
        function,
        &sequences,
        &mut caller,
    );
    values.clear();
    keep_result(&mut scope, result)
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
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let function = word_at(&scope, &handles, 0)?;
    callback_word(scope.context(), function)?;
    let rooted = words(&scope, &handles);
    let mut caller = BuiltinFunctionCaller;
    let result = operation(
        scope.context_mut(),
        runtime,
        function,
        rooted.get(1..).ok_or(ObjectError::TypeError)?,
        &mut caller,
    );
    values.clear();
    keep_result(&mut scope, result)
}
pub(super) fn map_into_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let destination = word_at(&scope, &handles, 0)?;
    let function = word_at(&scope, &handles, 1)?;
    callback_word(scope.context(), function)?;
    let rooted = words(&scope, &handles);
    let sequences = hof_sequences(
        scope.context(),
        rooted.get(2..).ok_or(ObjectError::TypeError)?,
    )?;
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::map_into(
        scope.context_mut(),
        runtime,
        destination,
        function,
        &sequences,
        &mut caller,
    );
    values.clear();
    keep_result(&mut scope, result)
}
/// Parse the `:key`, `:initial-value`, `:from-end`, `:start`, and `:end`
/// keyword arguments that trail REDUCE's required function and sequence.
fn parse_reduce_options(
    ctx: &ThreadContext,
    args: &[Word],
) -> Result<domain::higher_order::ReduceOptions, ObjectError> {
    let mut options = domain::higher_order::ReduceOptions::default();
    if !args.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    let mut index = 0;
    while index < args.len() {
        let key_word = *args.get(index).ok_or(ObjectError::Layout)?;
        let ObjectRef::Symbol(symbol) = classify_object(ctx, key_word) else {
            return Err(ObjectError::TypeError);
        };
        let name = symbol_name(ctx, symbol)?;
        let name = (0..string_length(ctx, name)?)
            .map(|i| string_ref(ctx, name, i))
            .collect::<Result<String, _>>()?
            .to_ascii_uppercase();
        let value = *args.get(index + 1).ok_or(ObjectError::TypeError)?;
        match name.as_str() {
            "KEY" => options.key = (value != Word::NIL).then_some(value),
            "INITIAL-VALUE" => options.initial_value = Some(value),
            "FROM-END" => options.from_end = value != Word::NIL,
            "START" => {
                options.start = usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                    .map_err(|_| ObjectError::TypeError)?;
            }
            "END" => {
                options.end = if value == Word::NIL {
                    None
                } else {
                    Some(
                        usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                            .map_err(|_| ObjectError::TypeError)?,
                    )
                };
            }
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown REDUCE keywords
        }
        index += 2;
    }
    Ok(options)
}
pub(super) fn reduce_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let function = word_at(&scope, &handles, 0)?;
    let sequence_word = word_at(&scope, &handles, 1)?;
    let sequence = sequence_arg(scope.context(), sequence_word)?;
    callback_word(scope.context(), function)?;
    let rooted = words(&scope, &handles);
    let options = parse_reduce_options(
        scope.context(),
        rooted.get(2..).ok_or(ObjectError::TypeError)?,
    )?;
    let mut caller = BuiltinFunctionCaller;
    let result = domain::higher_order::reduce(
        scope.context_mut(),
        runtime,
        function,
        sequence,
        options,
        &mut caller,
    );
    values.clear();
    keep_result(&mut scope, result)
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
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let rooted = words(&scope, &handles);
    let (positional, options) = domain::order_sets::parse_options(scope.context(), &rooted, 2)?;
    let first = *positional.first().ok_or(ObjectError::TypeError)?;
    let second = *positional.get(1).ok_or(ObjectError::TypeError)?;
    values.clear();
    let result = operation(scope.context_mut(), runtime, first, second, options);
    keep_result(&mut scope, result)
}
pub(super) fn set_sort_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
    stable: bool,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let rooted = words(&scope, &handles);
    let (positional, options) = domain::order_sets::parse_options(scope.context(), &rooted, 2)?;
    let sequence = *positional.first().ok_or(ObjectError::TypeError)?;
    let predicate = *positional.get(1).ok_or(ObjectError::TypeError)?;
    let result = domain::order_sets::sort(
        scope.context_mut(),
        runtime,
        sequence,
        predicate,
        options.key,
        stable,
    );
    values.clear();
    keep_result(&mut scope, result)
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

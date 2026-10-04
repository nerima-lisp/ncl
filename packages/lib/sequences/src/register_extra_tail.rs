use super::{
    BuiltinArgs, BuiltinFunctionCaller, MultipleValues, ObjectError, Runtime, Scope, ThreadContext,
    Word, callback_word, domain, keep_result, list_map_entry, order_set_entry, root_args,
    selection_parse, sequence_arg, word_at, words,
};
use ncl_object::{ObjectRef, car as object_car, cdr as object_cdr, classify_object, string_length,
    string_ref, symbol_name};

pub fn subst_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let replacement = args.get(0).ok_or(ObjectError::TypeError)?;
    let old = args.get(1).ok_or(ObjectError::TypeError)?;
    let tree = args.get(2).ok_or(ObjectError::TypeError)?;

    fn substitute(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        replacement: Word,
        old: Word,
        tree: Word,
    ) -> Result<Word, ObjectError> {
        if domain::equality::equal(ctx, tree, old)? {
            return Ok(replacement);
        }
        if !matches!(classify_object(ctx, tree), ObjectRef::Cons(_)) {
            return Ok(tree);
        }
        let car = substitute(ctx, runtime, replacement, old, object_car(ctx, tree)?)?;
        let cdr = substitute(ctx, runtime, replacement, old, object_cdr(ctx, tree)?)?;
        let mut scope = Scope::new(ctx);
        let car = scope.root(ncl_object::Local::from_word(car));
        let cdr = scope.root(ncl_object::Local::from_word(cdr));
        let result = scope.make_cons(runtime, car, cdr)?;
        Ok(scope.get(result).as_word())
    }

    substitute(ctx, runtime, replacement, old, tree)
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
pub fn reduce_entry(
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

/// Shared implementation for `REMOVE-IF`/`REMOVE-IF-NOT` (`has_replacement`
/// false) and `SUBSTITUTE-IF`/`SUBSTITUTE-IF-NOT` (`has_replacement` true).
/// `negate` selects the `-IF-NOT` sense.
pub fn selection_transform_if_entry(
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
pub fn remove_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_if_entry(ctx, runtime, args, values, false, false)
}
pub fn remove_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_if_entry(ctx, runtime, args, values, false, true)
}
pub fn substitute_if_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_if_entry(ctx, runtime, args, values, true, false)
}
pub fn substitute_if_not_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    selection_transform_if_entry(ctx, runtime, args, values, true, true)
}

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

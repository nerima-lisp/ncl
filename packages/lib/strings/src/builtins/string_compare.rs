fn string_compare_builtin(
    ctx: &ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
    predicate: fn(std::cmp::Ordering) -> bool,
    fold: bool,
) -> Result<Word, ObjectError> {
    let left = string_compare(ctx, args, fold)?;
    let right = string_compare_word(ctx, args.required(1)?, fold)?;
    Ok(if predicate(left.as_slice().cmp(right.as_slice())) {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn string_equal_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering == std::cmp::Ordering::Equal,
        false,
    )
}
fn string_not_equal_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering != std::cmp::Ordering::Equal,
        false,
    )
}
fn string_less_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering == std::cmp::Ordering::Less,
        false,
    )
}
fn string_greater_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering == std::cmp::Ordering::Greater,
        false,
    )
}
fn string_not_greater_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering != std::cmp::Ordering::Greater,
        false,
    )
}
fn string_not_less_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering != std::cmp::Ordering::Less,
        false,
    )
}
fn string_equal_ci_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering == std::cmp::Ordering::Equal,
        true,
    )
}
fn string_not_equal_ci_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering != std::cmp::Ordering::Equal,
        true,
    )
}
fn string_less_ci_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering == std::cmp::Ordering::Less,
        true,
    )
}
fn string_greater_ci_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering == std::cmp::Ordering::Greater,
        true,
    )
}
fn string_not_greater_ci_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering != std::cmp::Ordering::Greater,
        true,
    )
}
fn string_not_less_ci_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_compare_builtin(
        ctx,
        runtime,
        args,
        values,
        |ordering| ordering != std::cmp::Ordering::Less,
        true,
    )
}

fn string_case_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    upper: bool,
) -> Result<Word, ObjectError> {
    let chars = string_chars(ctx, args.required(0)?)?;
    make_result_string(
        ctx,
        runtime,
        chars.into_iter().flat_map(|c| {
            if upper {
                c.to_uppercase().collect::<Vec<_>>()
            } else {
                c.to_lowercase().collect::<Vec<_>>()
            }
        }),
    )
}

fn string_upcase_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_case_builtin(ctx, runtime, args, true)
}
fn string_downcase_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    string_case_builtin(ctx, runtime, args, false)
}
fn string_capitalize_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut start = true;
    let chars = string_chars(ctx, args.required(0)?)?;
    make_result_string(
        ctx,
        runtime,
        chars.into_iter().flat_map(|character| {
            let mapped = if start {
                character.to_uppercase().collect::<Vec<_>>()
            } else {
                character.to_lowercase().collect::<Vec<_>>()
            };
            start = !character.is_alphanumeric();
            mapped
        }),
    )
}

fn make_string_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let size = usize::try_from(
        args.required(0)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    let initial = args
        .get(1)
        .map_or(Ok(' '), character)
        .map_err(|_| ObjectError::TypeError)?;
    make_result_string(ctx, runtime, std::iter::repeat_n(initial, size))
}

fn trim_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    side: u8,
) -> Result<Word, ObjectError> {
    let bag = string_chars(ctx, args.required(0)?)?;
    let input = string_chars(ctx, args.required(1)?)?;
    let mut start = 0;
    let mut end = input.len();
    if side & 1 != 0 {
        while start < end && bag.contains(&input[start]) {
            start += 1;
        }
    }
    if side & 2 != 0 {
        while end > start && bag.contains(&input[end - 1]) {
            end -= 1;
        }
    }
    make_result_string(ctx, runtime, input[start..end].iter().copied())
}
fn string_trim_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    trim_builtin(ctx, runtime, args, 3)
}
fn string_left_trim_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    trim_builtin(ctx, runtime, args, 1)
}
fn string_right_trim_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    trim_builtin(ctx, runtime, args, 2)
}

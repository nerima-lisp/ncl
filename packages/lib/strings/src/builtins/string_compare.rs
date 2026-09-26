fn string_compare_builtin(
    ctx: &ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
    comparison: StringComparison,
    fold: bool,
) -> Result<Word, ObjectError> {
    let (left, right) = string_compare_values(ctx, args, fold)?;
    let ordering = left.as_slice().cmp(right.as_slice());
    let index = left
        .iter()
        .zip(&right)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| left.len().min(right.len()));
    let result = match comparison {
        StringComparison::Equal => ordering == std::cmp::Ordering::Equal,
        StringComparison::NotEqual => ordering != std::cmp::Ordering::Equal,
        StringComparison::Less => ordering == std::cmp::Ordering::Less,
        StringComparison::Greater => ordering == std::cmp::Ordering::Greater,
        StringComparison::NotGreater => ordering != std::cmp::Ordering::Greater,
        StringComparison::NotLess => ordering != std::cmp::Ordering::Less,
    };
    if !result {
        return Ok(Word::NIL);
    }
    if matches!(comparison, StringComparison::Equal | StringComparison::NotGreater | StringComparison::NotLess)
    {
        Ok(Word::TRUE)
    } else {
        Ok(Word::fixnum(i64::try_from(index).map_err(|_| ObjectError::TypeError)?))
    }
}

#[derive(Clone, Copy)]
enum StringComparison {
    Equal,
    NotEqual,
    Less,
    Greater,
    NotGreater,
    NotLess,
}

fn string_compare_values(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    fold: bool,
) -> Result<(Vec<char>, Vec<char>), ObjectError> {
    let left_value = args.required(0)?;
    let right_value = args.required(1)?;
    let left_chars = string_chars(ctx, left_value)?;
    let right_chars = string_chars(ctx, right_value)?;
    let mut left_start = 0;
    let mut left_end = left_chars.len();
    let mut right_start = 0;
    let mut right_end = right_chars.len();
    let words = args.as_slice();
    let mut index = 2;
    while index < words.len() {
        let name = keyword_name(ctx, words[index])?;
        let value = words.get(index + 1).ok_or(ObjectError::TypeError)?;
        let number = usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::TypeError)?;
        match name.as_str() {
            "START" | "START1" => left_start = number,
            "END" | "END1" => left_end = number,
            "START2" => right_start = number,
            "END2" => right_end = number,
            _ => return Err(ObjectError::TypeError),
        }
        index += 2;
    }
    if left_start > left_end
        || left_end > left_chars.len()
        || right_start > right_end
        || right_end > right_chars.len()
    {
        return Err(ObjectError::TypeError);
    }
    let left = if fold {
        left_chars[left_start..left_end]
            .iter()
            .copied()
            .flat_map(char::to_lowercase)
            .collect()
    } else {
        left_chars[left_start..left_end].to_vec()
    };
    let right = if fold {
        right_chars[right_start..right_end]
            .iter()
            .copied()
            .flat_map(char::to_lowercase)
            .collect()
    } else {
        right_chars[right_start..right_end].to_vec()
    };
    Ok((left, right))
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
        StringComparison::Equal,
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
        StringComparison::NotEqual,
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
        StringComparison::Less,
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
        StringComparison::Greater,
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
        StringComparison::NotGreater,
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
        StringComparison::NotLess,
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
        StringComparison::Equal,
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
        StringComparison::NotEqual,
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
        StringComparison::Less,
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
        StringComparison::Greater,
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
        StringComparison::NotGreater,
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
        StringComparison::NotLess,
        true,
    )
}

fn string_case_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    upper: bool,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let chars = string_chars(ctx, value)?;
    let (start, end) = string_range(ctx, args, 1, chars.len(), &["START"], &["END"])?;
    make_result_string(
        ctx,
        runtime,
        chars[start..end].iter().copied().flat_map(|c| {
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
    let (range_start, range_end) =
        string_range(ctx, args, 1, chars.len(), &["START"], &["END"])?;
    make_result_string(
        ctx,
        runtime,
        chars[range_start..range_end]
            .iter()
            .copied()
            .flat_map(|character| {
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

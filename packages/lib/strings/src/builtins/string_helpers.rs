fn char_equal_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(&builtin_words(a), |x, y| x == y)
}
fn char_not_equal_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(&builtin_words(a), |x, y| x != y)
}
fn char_less_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(&builtin_words(a), |x, y| x < y)
}
fn char_greater_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(&builtin_words(a), |x, y| x > y)
}
fn char_not_greater_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(&builtin_words(a), |x, y| x <= y)
}
fn char_not_less_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(&builtin_words(a), |x, y| x >= y)
}
fn char_equal_ci_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(&builtin_words(a), |x, y| x == y)
}
fn char_not_equal_ci_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(&builtin_words(a), |x, y| x != y)
}
fn char_less_ci_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(&builtin_words(a), |x, y| x < y)
}
fn char_greater_ci_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(&builtin_words(a), |x, y| x > y)
}
fn char_not_greater_ci_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(&builtin_words(a), |x, y| x <= y)
}
fn char_not_less_ci_builtin(
    _: &mut ThreadContext,
    _: &Runtime,
    a: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(&builtin_words(a), |x, y| x >= y)
}

fn string_designator(ctx: &ThreadContext, value: Word) -> Result<Word, ObjectError> {
    match ncl_object::classify_object(ctx, value) {
        ncl_object::ObjectRef::String(_) => Ok(value),
        ncl_object::ObjectRef::Symbol(_) => ncl_object::symbol_name(ctx, value),
        _ => Err(ObjectError::TypeError),
    }
}

fn string_chars(ctx: &ThreadContext, value: Word) -> Result<Vec<char>, ObjectError> {
    if value.is_character() {
        return Ok(vec![character(value)?]);
    }
    let string = string_designator(ctx, value)?;
    let length = ncl_object::string_length(ctx, string)?;
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, string, index))
        .collect()
}

fn string_index(args: &BuiltinArgs<'_>) -> Result<usize, ObjectError> {
    usize::try_from(
        args.required(1)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)
}

fn keyword_name(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    let ncl_object::ObjectRef::Symbol(symbol) = ncl_object::classify_object(ctx, value) else {
        return Err(ObjectError::TypeError);
    };
    let name = ncl_object::symbol_name(ctx, symbol)?;
    (0..ncl_object::string_length(ctx, name)?)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect()
}

fn string_range(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    option_start: usize,
    length: usize,
    start_names: &[&str],
    end_names: &[&str],
) -> Result<(usize, usize), ObjectError> {
    let mut start = 0;
    let mut end = length;
    let words = args.as_slice();
    let mut index = option_start;
    while index < words.len() {
        let name = keyword_name(ctx, *words.get(index).ok_or(ObjectError::TypeError)?)?;
        let value = words.get(index + 1).ok_or(ObjectError::TypeError)?;
        let number = usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::TypeError)?;
        if start_names.contains(&name.as_str()) {
            start = number;
        } else if end_names.contains(&name.as_str()) {
            end = number;
        } else {
            return Err(ObjectError::TypeError);
        }
        index += 2;
    }
    if start > end || end > length {
        return Err(ObjectError::TypeError);
    }
    Ok((start, end))
}

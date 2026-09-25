/// Return the Unicode `General_Category` abbreviation for a scalar value.
pub fn general_category(codepoint: u32) -> Option<&'static str> {
    if codepoint > 0x10_FFFF || (0xD800..=0xDFFF).contains(&codepoint) {
        return None;
    }
    unicode_data::GENERAL_CATEGORY_RANGES
        .iter()
        .find_map(|ranges| {
            ranges
                .binary_search_by(|(start, end, _)| {
                    if codepoint < *start {
                        std::cmp::Ordering::Greater
                    } else if codepoint > *end {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .ok()
                .map(|index| ranges[index].2)
        })
}

/// Return whether a word is a Lisp character.
#[must_use]
pub const fn characterp(value: Word) -> Word {
    if value.is_character() {
        Word::TRUE
    } else {
        Word::NIL
    }
}

fn character(value: Word) -> Result<char, ObjectError> {
    let value = typed_character(value).map_err(|_| ObjectError::TypeError)?;
    char::from_u32(value.value()).ok_or(ObjectError::TypeError)
}

fn typed_character(value: Word) -> Result<Character, LispError> {
    Character::try_from_word(value).map_err(LispError::from)
}

fn char_code_typed(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    value: Character,
) -> Result<Word, LispError> {
    Ok(Word::fixnum(i64::from(value.value())))
}

fn char_code_typed_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = typed_character(args.required(0)?).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })?;
    let result = char_code_typed(ctx, runtime, value).map_err(|error| {
        ctx.set_pending_lisp_error(error);
        ObjectError::TypeError
    })?;
    values.clear();
    Ok(result)
}

#[allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)]
fn characterp_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(characterp(args.required(0)?))
}

fn code_char_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let code = args
        .required(0)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let code = u32::try_from(code).map_err(|_| ObjectError::TypeError)?;
    char::from_u32(code)
        .map(|character| Word::character(character as u32))
        .ok_or(ObjectError::TypeError)
}

fn character_arg(value: Word) -> Result<char, ObjectError> {
    character(value)
}

fn builtin_words(args: &BuiltinArgs<'_>) -> Vec<Word> {
    (0..args.len())
        .filter_map(|index| args.get(index))
        .collect()
}

fn char_predicate<F>(args: &[Word], predicate: F) -> Result<Word, ObjectError>
where
    F: Fn(char, char) -> bool,
{
    if args.len() < 2 {
        return Err(ObjectError::TypeError);
    }
    let first = character_arg(args[0])?;
    let mut previous = first;
    for &arg in &args[1..] {
        let current = character_arg(arg)?;
        if !predicate(previous, current) {
            return Ok(Word::NIL);
        }
        previous = current;
    }
    Ok(Word::TRUE)
}

fn char_compare_builtin<F>(args: &[Word], predicate: F) -> Result<Word, ObjectError>
where
    F: Fn(char, char) -> bool,
{
    char_predicate(args, predicate)
}

fn char_casefold(value: char) -> char {
    value.to_lowercase().next().unwrap_or(value)
}

fn char_case_compare<F>(args: &[Word], predicate: F) -> Result<Word, ObjectError>
where
    F: Fn(char, char) -> bool,
{
    char_compare_builtin(args, |a, b| predicate(char_casefold(a), char_casefold(b)))
}

fn simple_char_builtin<F>(args: &[Word], map: F) -> Result<Word, ObjectError>
where
    F: Fn(char) -> char,
{
    Ok(Word::character(map(character_arg(args[0])?) as u32))
}

fn alpha_char_p_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if character_arg(args.required(0)?)?.is_alphabetic() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn alphanumericp_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if character_arg(args.required(0)?)?.is_alphanumeric() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn upper_case_p_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if character_arg(args.required(0)?)?.is_uppercase() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn lower_case_p_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if character_arg(args.required(0)?)?.is_lowercase() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn both_case_p_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let c = character_arg(args.required(0)?)?;
    Ok(if c.is_uppercase() || c.is_lowercase() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn digit_char_p_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let c = character_arg(args.required(0)?)?;
    Ok(c.to_digit(36)
        .map_or(Word::NIL, |n| Word::fixnum(i64::from(n))))
}
fn char_upcase_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    simple_char_builtin(&builtin_words(args), |c| {
        c.to_uppercase().next().unwrap_or(c)
    })
}
fn char_downcase_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    simple_char_builtin(&builtin_words(args), |c| {
        c.to_lowercase().next().unwrap_or(c)
    })
}
fn char_int_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_code_typed_entry(ctx, runtime, args, values)
}
fn general_category_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let category =
        general_category(character_arg(args.required(0)?)? as u32).ok_or(ObjectError::TypeError)?;
    ncl_object::make_string(ctx, runtime, &category.chars().collect::<Vec<_>>())
}

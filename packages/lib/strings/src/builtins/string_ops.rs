fn char_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = string_designator(ctx, args.required(0)?)?;
    Ok(Word::character(
        string_ref(ctx, string, string_index(args)?)? as u32,
    ))
}

fn schar_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_builtin(ctx, runtime, args, values)
}

fn named_character(name: &str) -> Option<char> {
    match name {
        "NULL" | "NUL" => Some('\0'),
        "BELL" => Some('\u{7}'),
        "BACKSPACE" => Some('\u{8}'),
        "TAB" => Some('\t'),
        "LINEFEED" | "NEWLINE" => Some('\n'),
        "PAGE" => Some('\u{c}'),
        "RETURN" => Some('\r'),
        "ESCAPE" => Some('\u{1b}'),
        "SPACE" => Some(' '),
        "RUBOUT" | "DELETE" => Some('\u{7f}'),
        _ => None,
    }
}

fn char_name_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = character(args.required(0)?)?;
    let name = match value {
        '\0' => Some("NULL"),
        '\u{7}' => Some("BELL"),
        '\u{8}' => Some("BACKSPACE"),
        '\t' => Some("TAB"),
        '\n' => Some("LINEFEED"),
        '\u{c}' => Some("PAGE"),
        '\r' => Some("RETURN"),
        '\u{1b}' => Some("ESCAPE"),
        ' ' => Some("SPACE"),
        '\u{7f}' => Some("RUBOUT"),
        _ => None,
    };
    name.map_or(Ok(Word::NIL), |name| {
        make_result_string(ctx, runtime, name.chars())
    })
}

fn name_char_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name: String = string_chars(ctx, args.required(0)?)?.into_iter().collect();
    let uppercase = name.to_ascii_uppercase();
    Ok(named_character(&uppercase).map_or(Word::NIL, |value| Word::character(value as u32)))
}

fn digit_char_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let digit = args
        .required(0)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let radix = args.get(1).map_or(Ok(10), |value| {
        value.as_fixnum().ok_or(ObjectError::TypeError)
    })?;
    if !(2..=36).contains(&radix) {
        return Err(ObjectError::TypeError);
    }
    if !(0..radix).contains(&digit) {
        return Ok(Word::NIL);
    }
    let value = u8::try_from(digit).map_err(|_| ObjectError::TypeError)?;
    Ok(Word::character(if value < 10 {
        u32::from(b'0' + value)
    } else {
        u32::from(b'A' + value - 10)
    }))
}

fn graphic_char_p_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = character(args.required(0)?)?;
    Ok(if !value.is_control() && value != '\u{7f}' {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn standard_char_p_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = character(args.required(0)?)?;
    Ok(if value == '\n' || (' '..='~').contains(&value) {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn nstring_case_builtin(
    ctx: &mut ThreadContext,
    args: &BuiltinArgs<'_>,
    map: fn(char) -> char,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let length = string_length(ctx, string)?;
    for index in 0..length {
        let mapped = map(string_ref(ctx, string, index)?);
        string_set(ctx, string, index, mapped)?;
    }
    Ok(string)
}

fn nstring_upcase_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    nstring_case_builtin(ctx, args, |value| {
        value.to_uppercase().next().unwrap_or(value)
    })
}

fn nstring_downcase_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    nstring_case_builtin(ctx, args, |value| {
        value.to_lowercase().next().unwrap_or(value)
    })
}

fn nstring_capitalize_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let string = args.required(0)?;
    let length = string_length(ctx, string)?;
    let mut start = true;
    for index in 0..length {
        let value = string_ref(ctx, string, index)?;
        let mapped = if start {
            value.to_uppercase().next().unwrap_or(value)
        } else {
            value.to_lowercase().next().unwrap_or(value)
        };
        string_set(ctx, string, index, mapped)?;
        start = !value.is_alphanumeric();
    }
    Ok(string)
}

fn simple_string_p_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if matches!(
            ncl_object::classify_object(ctx, args.required(0)?),
            ncl_object::ObjectRef::String(_)
        ) {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

fn make_result_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    chars: impl IntoIterator<Item = char>,
) -> Result<Word, ObjectError> {
    ncl_object::make_string(ctx, runtime, &chars.into_iter().collect::<Vec<_>>())
}

fn stringp_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if matches!(
            ncl_object::classify_object(_ctx, args.required(0)?),
            ncl_object::ObjectRef::String(_)
        ) {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

fn character_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    if value.is_character() {
        return Ok(value);
    }
    let chars = string_chars(ctx, value)?;
    if chars.len() == 1 {
        Ok(Word::character(chars[0] as u32))
    } else {
        Err(ObjectError::TypeError)
    }
}

fn string_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    match ncl_object::classify_object(ctx, value) {
        ncl_object::ObjectRef::String(_) | ncl_object::ObjectRef::Symbol(_) => {
            Ok(string_designator(ctx, value)?)
        }
        ncl_object::ObjectRef::Character(_) => {
            make_result_string(ctx, runtime, [character(value)?])
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn string_compare(
    ctx: &ThreadContext,
    args: &BuiltinArgs<'_>,
    fold: bool,
) -> Result<Vec<char>, ObjectError> {
    let chars = string_chars(ctx, args.required(0)?)?;
    if fold {
        Ok(chars.into_iter().flat_map(|c| c.to_lowercase()).collect())
    } else {
        Ok(chars)
    }
}

fn string_compare_word(
    ctx: &ThreadContext,
    value: Word,
    fold: bool,
) -> Result<Vec<char>, ObjectError> {
    let chars = string_chars(ctx, value)?;
    if fold {
        Ok(chars.into_iter().flat_map(|c| c.to_lowercase()).collect())
    } else {
        Ok(chars)
    }
}

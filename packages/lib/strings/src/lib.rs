//! Characters, strings, and the NCL-UNICODE data boundary.

mod unicode_data;

use ncl_object::{
    make_simple_vector, make_string, simple_vector_length, simple_vector_ref, string_length,
    string_ref, string_set, Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError,
    Parameter, ParameterType, Runtime, ThreadContext, Word,
};

/// Return the Unicode `General_Category` abbreviation for a scalar value.
#[must_use]
pub fn general_category(codepoint: u32) -> Option<&'static str> {
    if codepoint > 0x10_FFFF || (0xD800..=0xDFFF).contains(&codepoint) {
        return None;
    }
    unicode_data::GENERAL_CATEGORY_RANGES
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
        .map(|index| unicode_data::GENERAL_CATEGORY_RANGES[index].2)
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
    if !value.is_character() {
        return Err(ObjectError::TypeError);
    }
    char::from_u32(u32::try_from(value.bits() >> 4).map_err(|_| ObjectError::TypeError)?)
        .ok_or(ObjectError::TypeError)
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

fn char_code_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        i64::from(character(args.required(0)?)? as u32),
    ))
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
    char_code_builtin(ctx, runtime, args, values)
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

fn unicode_string_chars(ctx: &ThreadContext, value: Word) -> Result<Vec<char>, ObjectError> {
    (0..string_length(ctx, value)?)
        .map(|index| string_ref(ctx, value, index))
        .collect()
}

fn make_text(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    chars: Vec<char>,
) -> Result<Word, ObjectError> {
    make_string(ctx, runtime, &chars)
}

fn decompose_char(value: char, compatibility: bool, output: &mut Vec<char>) {
    let codepoint = value as u32;
    if (0xAC00..0xD7A4).contains(&codepoint) {
        let index = codepoint - 0xAC00;
        output.push(char::from_u32(0x1100 + index / 588).unwrap_or(value));
        output.push(char::from_u32(0x1161 + (index % 588) / 28).unwrap_or(value));
        if index % 28 != 0 {
            output.push(char::from_u32(0x11A7 + index % 28).unwrap_or(value));
        }
        return;
    }
    if let Some((_, is_compatibility, values)) = unicode_data::DECOMPOSITIONS
        .iter()
        .find(|entry| entry.0 == codepoint)
    {
        if compatibility || !is_compatibility {
            for &part in *values {
                if let Some(part) = char::from_u32(part) {
                    decompose_char(part, compatibility, output);
                }
            }
            return;
        }
    }
    output.push(value);
}

fn combining_class(value: char) -> u8 {
    unicode_data::COMBINING_CLASSES
        .iter()
        .find(|entry| entry.0 == value as u32)
        .map_or(0, |entry| entry.1)
}

fn normalize(chars: &[char], compatibility: bool, compose: bool) -> Vec<char> {
    let mut decomposed = Vec::new();
    for &value in chars {
        decompose_char(value, compatibility, &mut decomposed);
    }
    let mut reordered = Vec::with_capacity(decomposed.len());
    for value in decomposed {
        let class = combining_class(value);
        if class == 0 {
            reordered.push(value);
        } else {
            let mut position = reordered.len();
            while position > 0 && combining_class(reordered[position - 1]) > class {
                position -= 1;
            }
            reordered.insert(position, value);
        }
    }
    if !compose {
        return reordered;
    }
    let mut result = Vec::with_capacity(reordered.len());
    for value in reordered {
        if let Some(&starter) = result.last() {
            if combining_class(value) != 0 {
                if let Some((_, _, composed)) = unicode_data::COMPOSITIONS
                    .iter()
                    .find(|entry| entry.0 == starter as u32 && entry.1 == value as u32)
                {
                    result.pop();
                    result.push(char::from_u32(*composed).unwrap_or(value));
                    continue;
                }
            }
        }
        result.push(value);
    }
    result
}

fn transform_string<F>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    map: F,
) -> Result<Word, ObjectError>
where
    F: Fn(&[char]) -> Vec<char>,
{
    make_text(
        ctx,
        runtime,
        map(&unicode_string_chars(ctx, args.required(0)?)?),
    )
}

fn normalize_nfc_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| normalize(s, false, true))
}
fn normalize_nfd_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| normalize(s, false, false))
}
fn normalize_nfkc_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| normalize(s, true, true))
}
fn normalize_nfkd_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| normalize(s, true, false))
}
fn full_upcase_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        s.iter().flat_map(|c| c.to_uppercase()).collect()
    })
}
fn full_downcase_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        s.iter().flat_map(|c| c.to_lowercase()).collect()
    })
}
fn full_titlecase_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        let mut start = true;
        s.iter()
            .flat_map(|c| {
                let out: Vec<char> = if start {
                    c.to_uppercase().collect()
                } else {
                    c.to_lowercase().collect()
                };
                start = c.is_whitespace() || c.is_ascii_punctuation();
                out
            })
            .collect()
    })
}

fn string_to_utf8_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let bytes = unicode_string_chars(ctx, args.required(0)?)?
        .iter()
        .collect::<String>()
        .into_bytes();
    make_simple_vector(
        ctx,
        runtime,
        &bytes
            .into_iter()
            .map(|byte| Word::fixnum(i64::from(byte)))
            .collect::<Vec<_>>(),
    )
}
fn utf8_to_string_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let vector = args.required(0)?;
    let mut bytes = Vec::with_capacity(simple_vector_length(ctx, vector)?);
    for index in 0..simple_vector_length(ctx, vector)? {
        bytes.push(
            u8::try_from(
                simple_vector_ref(ctx, vector, index)?
                    .as_fixnum()
                    .ok_or(ObjectError::TypeError)?,
            )
            .map_err(|_| ObjectError::TypeError)?,
        );
    }
    let text = String::from_utf8(bytes).map_err(|_| ObjectError::TypeError)?;
    make_text(ctx, runtime, text.chars().collect())
}

fn grapheme_boundaries_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let chars = unicode_string_chars(ctx, args.required(0)?)?;
    let mut boundaries = vec![Word::fixnum(0)];
    let mut regional_count = 0usize;
    for (index, &current) in chars.iter().enumerate() {
        let previous = index.checked_sub(1).and_then(|i| chars.get(i)).copied();
        let extend = general_category(current as u32)
            .is_some_and(|category| matches!(category, "Mn" | "Mc" | "Me"))
            || current == '\u{200D}'
            || (0xFE00..=0xFE0F).contains(&(current as u32));
        let regional = (0x1F1E6..=0x1F1FF).contains(&(current as u32));
        if previous
            .is_some_and(|p| !extend && p != '\u{200D}' && (!regional || regional_count % 2 == 0))
        {
            boundaries.push(Word::fixnum(
                i64::try_from(index).map_err(|_| ObjectError::Layout)?,
            ));
        }
        regional_count = if regional { regional_count + 1 } else { 0 };
    }
    boundaries.push(Word::fixnum(
        i64::try_from(chars.len()).map_err(|_| ObjectError::Layout)?,
    ));
    make_simple_vector(ctx, runtime, &boundaries)
}

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

fn string_compare_builtin(
    ctx: &mut ThreadContext,
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

const CHARACTER: Parameter = Parameter {
    name: BuiltinName::new("CHARACTER"),
    ty: ParameterType::Any,
};
const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const CODE: Parameter = Parameter {
    name: BuiltinName::new("CODE"),
    ty: ParameterType::Fixnum,
};
const INDEX: Parameter = Parameter {
    name: BuiltinName::new("INDEX"),
    ty: ParameterType::Fixnum,
};
const DIGIT: Parameter = Parameter {
    name: BuiltinName::new("DIGIT"),
    ty: ParameterType::Fixnum,
};
const RADIX: Parameter = Parameter {
    name: BuiltinName::new("RADIX"),
    ty: ParameterType::Fixnum,
};
const STRING: Parameter = Parameter {
    name: BuiltinName::new("STRING"),
    ty: ParameterType::StringDesignator,
};
const STRINGS: &[Parameter] = &[STRING, STRING];
const REST_CHARACTER: Parameter = Parameter {
    name: BuiltinName::new("CHARACTERS"),
    ty: ParameterType::Any,
};
const STRING_ARGS: &[Parameter] = &[STRING];
const TRIM_ARGS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("CHAR-BAG"),
        ty: ParameterType::StringDesignator,
    },
    STRING,
];
const SIZE: Parameter = Parameter {
    name: BuiltinName::new("SIZE"),
    ty: ParameterType::Fixnum,
};
const INITIAL_ELEMENT: Parameter = Parameter {
    name: BuiltinName::new("INITIAL-ELEMENT"),
    ty: ParameterType::Any,
};

fn descriptor(lambda_list: LambdaList) -> Builtin {
    Builtin {
        convention: if lambda_list.is_direct() {
            BuiltinConvention::Direct(Arity::exact(lambda_list.required.len() as u8))
        } else {
            BuiltinConvention::Adapted
        },
        lambda_list,
    }
}

fn identity_adapter(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Ok((0..args.len())
        .filter_map(|index| args.get(index))
        .collect())
}

/// Register the Common Lisp character and string builtins owned by this crate.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    macro_rules! register {
        ($name:literal, $params:expr, $function:ident) => {
            let descriptor = descriptor($params);
            let implementation = if descriptor.lambda_list.is_direct() {
                BuiltinImplementation::direct(descriptor, $function)
            } else {
                BuiltinImplementation::adapted(descriptor, $function, identity_adapter)
            };
            runtime.register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new($name)),
                implementation,
            )?;
        };
    }
    register!(
        "CHARACTERP",
        LambdaList::fixed(&[OBJECT]),
        characterp_builtin
    );
    register!("CHAR", LambdaList::fixed(&[STRING, INDEX]), char_builtin);
    register!("SCHAR", LambdaList::fixed(&[STRING, INDEX]), schar_builtin);
    register!(
        "CHAR-CODE",
        LambdaList::fixed(&[CHARACTER]),
        char_code_builtin
    );
    register!(
        "CHAR-NAME",
        LambdaList::fixed(&[CHARACTER]),
        char_name_builtin
    );
    register!("CODE-CHAR", LambdaList::fixed(&[CODE]), code_char_builtin);
    register!("NAME-CHAR", LambdaList::fixed(&[STRING]), name_char_builtin);
    register!(
        "DIGIT-CHAR",
        LambdaList::with_optional(&[DIGIT], &[RADIX]),
        digit_char_builtin
    );
    register!(
        "ALPHA-CHAR-P",
        LambdaList::fixed(&[CHARACTER]),
        alpha_char_p_builtin
    );
    register!(
        "ALPHANUMERICP",
        LambdaList::fixed(&[CHARACTER]),
        alphanumericp_builtin
    );
    register!(
        "UPPER-CASE-P",
        LambdaList::fixed(&[CHARACTER]),
        upper_case_p_builtin
    );
    register!(
        "LOWER-CASE-P",
        LambdaList::fixed(&[CHARACTER]),
        lower_case_p_builtin
    );
    register!(
        "BOTH-CASE-P",
        LambdaList::fixed(&[CHARACTER]),
        both_case_p_builtin
    );
    register!(
        "DIGIT-CHAR-P",
        LambdaList::fixed(&[CHARACTER]),
        digit_char_p_builtin
    );
    register!(
        "GRAPHIC-CHAR-P",
        LambdaList::fixed(&[CHARACTER]),
        graphic_char_p_builtin
    );
    register!(
        "STANDARD-CHAR-P",
        LambdaList::fixed(&[CHARACTER]),
        standard_char_p_builtin
    );
    register!(
        "CHAR-UPCASE",
        LambdaList::fixed(&[CHARACTER]),
        char_upcase_builtin
    );
    register!(
        "CHAR-DOWNCASE",
        LambdaList::fixed(&[CHARACTER]),
        char_downcase_builtin
    );
    register!(
        "CHAR-INT",
        LambdaList::fixed(&[CHARACTER]),
        char_int_builtin
    );
    register!(
        "CHAR=",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_equal_builtin
    );
    register!(
        "CHAR/=",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_equal_builtin
    );
    register!(
        "CHAR<",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_less_builtin
    );
    register!(
        "CHAR>",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_greater_builtin
    );
    register!(
        "CHAR<=",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_greater_builtin
    );
    register!(
        "CHAR>=",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_less_builtin
    );
    register!(
        "CHAR-EQUAL",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_equal_ci_builtin
    );
    register!(
        "CHAR-NOT-EQUAL",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_equal_ci_builtin
    );
    register!(
        "CHAR-LESSP",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_less_ci_builtin
    );
    register!(
        "CHAR-GREATERP",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_greater_ci_builtin
    );
    register!(
        "CHAR-NOT-GREATERP",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_greater_ci_builtin
    );
    register!(
        "CHAR-NOT-LESSP",
        LambdaList::with_rest(&[CHARACTER], REST_CHARACTER),
        char_not_less_ci_builtin
    );
    register!("STRINGP", LambdaList::fixed(&[OBJECT]), stringp_builtin);
    register!("STRING", LambdaList::fixed(&[OBJECT]), string_builtin);
    register!("STRING=", LambdaList::fixed(STRINGS), string_equal_builtin);
    register!(
        "STRING/=",
        LambdaList::fixed(STRINGS),
        string_not_equal_builtin
    );
    register!("STRING<", LambdaList::fixed(STRINGS), string_less_builtin);
    register!(
        "STRING>",
        LambdaList::fixed(STRINGS),
        string_greater_builtin
    );
    register!(
        "STRING<=",
        LambdaList::fixed(STRINGS),
        string_not_greater_builtin
    );
    register!(
        "STRING>=",
        LambdaList::fixed(STRINGS),
        string_not_less_builtin
    );
    register!(
        "STRING-EQUAL",
        LambdaList::fixed(STRINGS),
        string_equal_ci_builtin
    );
    register!(
        "STRING-NOT-EQUAL",
        LambdaList::fixed(STRINGS),
        string_not_equal_ci_builtin
    );
    register!(
        "STRING-LESSP",
        LambdaList::fixed(STRINGS),
        string_less_ci_builtin
    );
    register!(
        "STRING-GREATERP",
        LambdaList::fixed(STRINGS),
        string_greater_ci_builtin
    );
    register!(
        "STRING-NOT-GREATERP",
        LambdaList::fixed(STRINGS),
        string_not_greater_ci_builtin
    );
    register!(
        "STRING-NOT-LESSP",
        LambdaList::fixed(STRINGS),
        string_not_less_ci_builtin
    );
    register!(
        "STRING-UPCASE",
        LambdaList::fixed(STRING_ARGS),
        string_upcase_builtin
    );
    register!(
        "STRING-DOWNCASE",
        LambdaList::fixed(STRING_ARGS),
        string_downcase_builtin
    );
    register!(
        "STRING-CAPITALIZE",
        LambdaList::fixed(STRING_ARGS),
        string_capitalize_builtin
    );
    register!(
        "NSTRING-UPCASE",
        LambdaList::fixed(&[STRING]),
        nstring_upcase_builtin
    );
    register!(
        "NSTRING-DOWNCASE",
        LambdaList::fixed(&[STRING]),
        nstring_downcase_builtin
    );
    register!(
        "NSTRING-CAPITALIZE",
        LambdaList::fixed(&[STRING]),
        nstring_capitalize_builtin
    );
    register!(
        "SIMPLE-STRING-P",
        LambdaList::fixed(&[OBJECT]),
        simple_string_p_builtin
    );
    register!(
        "MAKE-STRING",
        LambdaList::with_optional(&[SIZE], &[INITIAL_ELEMENT]),
        make_string_builtin
    );
    register!(
        "STRING-TRIM",
        LambdaList::fixed(TRIM_ARGS),
        string_trim_builtin
    );
    register!(
        "STRING-LEFT-TRIM",
        LambdaList::fixed(TRIM_ARGS),
        string_left_trim_builtin
    );
    register!(
        "STRING-RIGHT-TRIM",
        LambdaList::fixed(TRIM_ARGS),
        string_right_trim_builtin
    );
    macro_rules! register_unicode {
        ($name:literal, $function:ident) => {
            let descriptor = descriptor(LambdaList::fixed(&[STRING]));
            runtime.register_builtin(
                &mut ctx,
                BuiltinIdentifier::new(BuiltinPackage::NclUnicode, BuiltinName::new($name)),
                BuiltinImplementation::direct(descriptor, $function),
            )?;
        };
    }
    register_unicode!("NORMALIZE-NFC", normalize_nfc_builtin);
    register_unicode!("NORMALIZE-NFD", normalize_nfd_builtin);
    register_unicode!("NORMALIZE-NFKC", normalize_nfkc_builtin);
    register_unicode!("NORMALIZE-NFKD", normalize_nfkd_builtin);
    register_unicode!("FULL-UPCASE", full_upcase_builtin);
    register_unicode!("FULL-DOWNCASE", full_downcase_builtin);
    register_unicode!("FULL-TITLECASE", full_titlecase_builtin);
    register_unicode!("STRING-TO-UTF8", string_to_utf8_builtin);
    register_unicode!("UTF8-TO-STRING", utf8_to_string_builtin);
    register_unicode!("GRAPHEME-BOUNDARIES", grapheme_boundaries_builtin);
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclUnicode,
            BuiltinName::new("GENERAL-CATEGORY"),
        ),
        BuiltinImplementation::direct(
            descriptor(LambdaList::fixed(&[CHARACTER])),
            general_category_builtin,
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::general_category;
    use ncl_object::{FunctionObject, Runtime, ThreadContext, Word};

    fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
        let function = runtime
            .function(ctx, "COMMON-LISP", name)
            .and_then(|word| FunctionObject::try_from(word).ok())
            .unwrap_or_else(|| panic!("missing builtin {name}"));
        runtime
            .call_builtin(ctx, function, args)
            .unwrap_or_else(|error| panic!("{name} failed: {error:?}"))
    }

    fn call_result(
        runtime: &Runtime,
        ctx: &mut ThreadContext,
        name: &str,
        args: &[Word],
    ) -> Result<Word, ncl_object::ObjectError> {
        let function = runtime
            .function(ctx, "COMMON-LISP", name)
            .and_then(|word| FunctionObject::try_from(word).ok())
            .unwrap_or_else(|| panic!("missing builtin {name}"));
        runtime.call_builtin(ctx, function, args)
    }

    #[test]
    fn generated_unicode_categories_cover_scalar_boundaries() {
        assert_eq!(general_category('A' as u32), Some("Lu"));
        assert_eq!(general_category('a' as u32), Some("Ll"));
        assert_eq!(general_category(0xD800), None);
        assert_eq!(general_category(0x11_0000), None);
    }

    #[test]
    fn string_builtins_cover_comparison_case_trim_and_construction() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("context: {error:?}"));
        super::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

        let hello = ncl_object::make_string(&mut ctx, &runtime, &['H', 'i'])
            .unwrap_or_else(|error| panic!("string: {error:?}"));
        let hi = ncl_object::make_string(&mut ctx, &runtime, &['h', 'i'])
            .unwrap_or_else(|error| panic!("string: {error:?}"));
        assert_eq!(
            call(&runtime, &mut ctx, "STRING-EQUAL", &[hello, hi]),
            Word::TRUE
        );

        let upper = call(&runtime, &mut ctx, "STRING-UPCASE", &[hi]);
        assert_eq!(ncl_object::string_length(&ctx, upper), Ok(2));
        assert_eq!(ncl_object::string_ref(&ctx, upper, 0), Ok('H'));

        let padded = ncl_object::make_string(&mut ctx, &runtime, &[' ', 'H', 'i', ' '])
            .unwrap_or_else(|error| panic!("string: {error:?}"));
        let spaces = ncl_object::make_string(&mut ctx, &runtime, &[' '])
            .unwrap_or_else(|error| panic!("string: {error:?}"));
        let trimmed = call(&runtime, &mut ctx, "STRING-TRIM", &[spaces, padded]);
        assert_eq!(ncl_object::string_ref(&ctx, trimmed, 0), Ok('H'));

        let made = call(
            &runtime,
            &mut ctx,
            "MAKE-STRING",
            &[Word::fixnum(3), Word::character('x' as u32)],
        );
        assert_eq!(ncl_object::string_ref(&ctx, made, 2), Ok('x'));
    }

    #[test]
    fn character_and_mutating_string_builtins_cover_boundaries() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("context: {error:?}"));
        super::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

        let source = ncl_object::make_string(&mut ctx, &runtime, &['a', 'b'])
            .unwrap_or_else(|error| panic!("string: {error:?}"));
        assert_eq!(
            call(&runtime, &mut ctx, "CHAR", &[source, Word::fixnum(1)]),
            Word::character('b' as u32)
        );
        assert_eq!(
            call(&runtime, &mut ctx, "SCHAR", &[source, Word::fixnum(0)]),
            Word::character('a' as u32)
        );
        assert_eq!(
            call_result(&runtime, &mut ctx, "CHAR", &[source, Word::fixnum(2)]),
            Err(ncl_object::ObjectError::TypeError)
        );
        assert_eq!(
            call_result(&runtime, &mut ctx, "CHAR", &[Word::TRUE, Word::fixnum(0)]),
            Err(ncl_object::ObjectError::TypeError)
        );

        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "CHAR-NAME",
                &[Word::character(' ' as u32)]
            ),
            ncl_object::make_string(&mut ctx, &runtime, &['S', 'P', 'A', 'C', 'E']).unwrap()
        );
        let name = ncl_object::make_string(&mut ctx, &runtime, &['t', 'a', 'b']).unwrap();
        assert_eq!(
            call(&runtime, &mut ctx, "NAME-CHAR", &[name]),
            Word::character('\t' as u32)
        );
        let unknown = ncl_object::make_string(&mut ctx, &runtime, &['N', 'O', 'P', 'E']).unwrap();
        assert_eq!(call(&runtime, &mut ctx, "NAME-CHAR", &[unknown]), Word::NIL);

        assert_eq!(
            call(&runtime, &mut ctx, "DIGIT-CHAR", &[Word::fixnum(15)]),
            Word::character('F' as u32)
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "DIGIT-CHAR",
                &[Word::fixnum(10), Word::fixnum(10)]
            ),
            Word::NIL
        );
        assert_eq!(
            call_result(
                &runtime,
                &mut ctx,
                "DIGIT-CHAR",
                &[Word::fixnum(1), Word::fixnum(1)]
            ),
            Err(ncl_object::ObjectError::TypeError)
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "GRAPHIC-CHAR-P",
                &[Word::character('A' as u32)]
            ),
            Word::TRUE
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "GRAPHIC-CHAR-P",
                &[Word::character('\n' as u32)]
            ),
            Word::NIL
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "STANDARD-CHAR-P",
                &[Word::character('~' as u32)]
            ),
            Word::TRUE
        );
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "STANDARD-CHAR-P",
                &[Word::character('\u{1b}' as u32)]
            ),
            Word::NIL
        );

        let mutable = ncl_object::make_string(
            &mut ctx,
            &runtime,
            &['h', 'I', ' ', 'T', 'H', 'E', 'R', 'E'],
        )
        .unwrap();
        assert_eq!(
            call(&runtime, &mut ctx, "NSTRING-UPCASE", &[mutable]),
            mutable
        );
        assert_eq!(ncl_object::string_ref(&ctx, mutable, 0), Ok('H'));
        assert_eq!(
            call(&runtime, &mut ctx, "NSTRING-DOWNCASE", &[mutable]),
            mutable
        );
        assert_eq!(ncl_object::string_ref(&ctx, mutable, 1), Ok('i'));
        assert_eq!(
            call(&runtime, &mut ctx, "NSTRING-CAPITALIZE", &[mutable]),
            mutable
        );
        assert_eq!(ncl_object::string_ref(&ctx, mutable, 0), Ok('H'));
        assert_eq!(ncl_object::string_ref(&ctx, mutable, 1), Ok('i'));
        assert_eq!(
            call(&runtime, &mut ctx, "SIMPLE-STRING-P", &[mutable]),
            Word::TRUE
        );
        assert_eq!(
            call(&runtime, &mut ctx, "SIMPLE-STRING-P", &[Word::TRUE]),
            Word::NIL
        );
    }
}

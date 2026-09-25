//! Characters, strings, and the NCL-UNICODE data boundary.

mod unicode_data;

use ncl_object::{
    Builtin, BuiltinImplementation, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
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
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(characterp(args[0]))
}

fn char_code_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(i64::from(character(args[0])? as u32)))
}

fn code_char_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let code = args[0].as_fixnum().ok_or(ObjectError::TypeError)?;
    let code = u32::try_from(code).map_err(|_| ObjectError::TypeError)?;
    char::from_u32(code)
        .map(|character| Word::character(character as u32))
        .ok_or(ObjectError::TypeError)
}

fn character_arg(value: Word) -> Result<char, ObjectError> {
    character(value)
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
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if character_arg(args[0])?.is_alphabetic() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn alphanumericp_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if character_arg(args[0])?.is_alphanumeric() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn upper_case_p_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if character_arg(args[0])?.is_uppercase() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn lower_case_p_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if character_arg(args[0])?.is_lowercase() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn both_case_p_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let c = character_arg(args[0])?;
    Ok(if c.is_uppercase() || c.is_lowercase() {
        Word::TRUE
    } else {
        Word::NIL
    })
}
fn digit_char_p_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let c = character_arg(args[0])?;
    Ok(c.to_digit(36)
        .map_or(Word::NIL, |n| Word::fixnum(i64::from(n))))
}
fn char_upcase_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    simple_char_builtin(args, |c| c.to_uppercase().next().unwrap_or(c))
}
fn char_downcase_builtin(
    _runtime: &Runtime,
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    simple_char_builtin(args, |c| c.to_lowercase().next().unwrap_or(c))
}
fn char_int_builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_code_builtin(runtime, ctx, args, values)
}
fn general_category_builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let category =
        general_category(character_arg(args[0])? as u32).ok_or(ObjectError::TypeError)?;
    ncl_object::make_string(ctx, runtime, &category.chars().collect::<Vec<_>>())
}

fn char_equal_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(a, |x, y| x == y)
}
fn char_not_equal_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(a, |x, y| x != y)
}
fn char_less_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(a, |x, y| x < y)
}
fn char_greater_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(a, |x, y| x > y)
}
fn char_not_greater_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(a, |x, y| x <= y)
}
fn char_not_less_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_compare_builtin(a, |x, y| x >= y)
}
fn char_equal_ci_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(a, |x, y| x == y)
}
fn char_not_equal_ci_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(a, |x, y| x != y)
}
fn char_less_ci_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(a, |x, y| x < y)
}
fn char_greater_ci_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(a, |x, y| x > y)
}
fn char_not_greater_ci_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(a, |x, y| x <= y)
}
fn char_not_less_ci_builtin(
    _: &Runtime,
    _: &mut ThreadContext,
    a: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    char_case_compare(a, |x, y| x >= y)
}

/// Register the implemented character builtins and Unicode entry point.
///
///
/// # Errors
///
/// Returns an allocation or symbol registration error.
#[allow(clippy::too_many_lines)]
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let characterp = BuiltinImplementation::direct(
        ncl_object::Builtin {
            arity: 1,
            direct: true,
            lambda_list: "object",
        },
        characterp_builtin,
    );
    runtime.register_builtin(&mut ctx, "COMMON-LISP", "CHARACTERP", characterp)?;
    let char_code = BuiltinImplementation::direct(
        ncl_object::Builtin {
            arity: 1,
            direct: true,
            lambda_list: "character",
        },
        char_code_builtin,
    );
    runtime.register_builtin(&mut ctx, "COMMON-LISP", "CHAR-CODE", char_code)?;
    let code_char = BuiltinImplementation::direct(
        ncl_object::Builtin {
            arity: 1,
            direct: true,
            lambda_list: "code",
        },
        code_char_builtin,
    );
    runtime.register_builtin(&mut ctx, "COMMON-LISP", "CODE-CHAR", code_char)?;
    let entries: &[(&str, Builtin, ncl_object::RustBuiltin)] = &[
        (
            "ALPHA-CHAR-P",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            alpha_char_p_builtin,
        ),
        (
            "ALPHANUMERICP",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            alphanumericp_builtin,
        ),
        (
            "UPPER-CASE-P",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            upper_case_p_builtin,
        ),
        (
            "LOWER-CASE-P",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            lower_case_p_builtin,
        ),
        (
            "BOTH-CASE-P",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            both_case_p_builtin,
        ),
        (
            "DIGIT-CHAR-P",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            digit_char_p_builtin,
        ),
        (
            "CHAR-UPCASE",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            char_upcase_builtin,
        ),
        (
            "CHAR-DOWNCASE",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            char_downcase_builtin,
        ),
        (
            "CHAR-INT",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            char_int_builtin,
        ),
        (
            "GENERAL-CATEGORY",
            Builtin {
                arity: 1,
                direct: true,
                lambda_list: "character",
            },
            general_category_builtin,
        ),
        (
            "CHAR=",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_equal_builtin,
        ),
        (
            "CHAR/=",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_not_equal_builtin,
        ),
        (
            "CHAR<",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_less_builtin,
        ),
        (
            "CHAR>",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_greater_builtin,
        ),
        (
            "CHAR<=",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_not_greater_builtin,
        ),
        (
            "CHAR>=",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_not_less_builtin,
        ),
        (
            "CHAR-EQUAL",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_equal_ci_builtin,
        ),
        (
            "CHAR-NOT-EQUAL",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_not_equal_ci_builtin,
        ),
        (
            "CHAR-LESSP",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_less_ci_builtin,
        ),
        (
            "CHAR-GREATERP",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_greater_ci_builtin,
        ),
        (
            "CHAR-NOT-GREATERP",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_not_greater_ci_builtin,
        ),
        (
            "CHAR-NOT-LESSP",
            Builtin {
                arity: 2,
                direct: false,
                lambda_list: "&rest characters",
            },
            char_not_less_ci_builtin,
        ),
    ];
    for &(name, descriptor, function) in entries {
        runtime.register_builtin(
            &mut ctx,
            "COMMON-LISP",
            name,
            BuiltinImplementation::direct(descriptor, function),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::general_category;

    #[test]
    fn generated_unicode_categories_cover_scalar_boundaries() {
        assert_eq!(general_category('A' as u32), Some("Lu"));
        assert_eq!(general_category('a' as u32), Some("Ll"));
        assert_eq!(general_category(0xD800), None);
        assert_eq!(general_category(0x11_0000), None);
    }
}

//! Characters, strings, and the NCL-UNICODE data boundary.

mod unicode_data;

use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

/// Return the Unicode General_Category abbreviation for a scalar value.
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
pub fn characterp(value: Word) -> Word {
    if value.is_character() { Word::TRUE } else { Word::NIL }
}

fn character(value: Word) -> Result<char, ObjectError> {
    if !value.is_character() {
        return Err(ObjectError::TypeError);
    }
    char::from_u32((value.bits() >> 4) as u32).ok_or(ObjectError::TypeError)
}

fn characterp_builtin(
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(characterp(args[0]))
}

fn char_code_builtin(
    _ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(i64::from(character(args[0])? as u32)))
}

fn code_char_builtin(
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

/// Register the first character builtins and the frozen Unicode entry point.
///
/// The remaining string callbacks are intentionally kept out of this partial
/// registration until the runtime call boundary can pass the allocating
/// `Runtime` handle required by `make_string`.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let characterp = ncl_object::BuiltinImplementation::direct(
        ncl_object::Builtin { arity: 1, direct: true, lambda_list: "object" },
        characterp_builtin,
    );
    runtime.register_builtin(&mut ctx, "COMMON-LISP", "CHARACTERP", characterp)?;
    let char_code = ncl_object::BuiltinImplementation::direct(
        ncl_object::Builtin { arity: 1, direct: true, lambda_list: "character" },
        char_code_builtin,
    );
    runtime.register_builtin(&mut ctx, "COMMON-LISP", "CHAR-CODE", char_code)?;
    let code_char = ncl_object::BuiltinImplementation::direct(
        ncl_object::Builtin { arity: 1, direct: true, lambda_list: "code" },
        code_char_builtin,
    );
    runtime.register_builtin(&mut ctx, "COMMON-LISP", "CODE-CHAR", code_char)?;
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

use super::{
    RuntimeError, Value, arity, character_argument, character_designator, exact, index_argument,
    integer_argument, out_of_bounds, string_designator, type_error,
};

pub(super) fn string_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "string", 1)?;
    Ok(Value::string(string_designator("string", &arguments[0])?))
}

pub(super) fn make_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("make-string", "1 or 2", arguments.len()));
    }
    let length = index_argument("make-string", &arguments[0])?;
    let initial = arguments
        .get(1)
        .map(|value| character_argument("make-string", value))
        .transpose()?
        .unwrap_or(' ');
    Ok(Value::string(
        std::iter::repeat_n(initial, length).collect::<String>(),
    ))
}

pub(super) fn character_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "character", 1)?;
    Ok(Value::Character(character_designator(
        "character",
        &arguments[0],
    )?))
}

pub(super) fn character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char", 2)?;
    let index = index_argument("char", &arguments[1])?;
    let Value::String(value) = &arguments[0] else {
        return Err(type_error("char", "string", &arguments[0]));
    };
    value
        .chars()
        .nth(index)
        .map(Value::Character)
        .ok_or_else(|| out_of_bounds("char", index))
}

pub(super) fn simple_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "schar", 2)?;
    let index = index_argument("schar", &arguments[1])?;
    let Value::String(value) = &arguments[0] else {
        return Err(type_error("schar", "simple-string", &arguments[0]));
    };
    value
        .chars()
        .nth(index)
        .map(Value::Character)
        .ok_or_else(|| out_of_bounds("schar", index))
}

pub(super) fn char_code(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-code", 1)?;
    Ok(Value::Integer(
        character_argument("char-code", &arguments[0])? as i64,
    ))
}

pub(super) fn char_int(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-int", 1)?;
    Ok(Value::Integer(
        character_argument("char-int", &arguments[0])? as i64,
    ))
}

pub(super) fn code_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "code-char", 1)?;
    code_char_value("code-char", &arguments[0])
}

pub(super) fn int_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "int-char", 1)?;
    code_char_value("int-char", &arguments[0])
}

pub(super) fn code_char_value(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let code = integer_argument(function, value)?;
    Ok(u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .map_or(Value::Nil, Value::Character))
}

pub(super) fn character_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char=", arguments, false, |left, right| left == right)
}

pub(super) fn character_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char/=", arguments, false)
}

pub(super) fn character_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-equal", arguments, true, |left, right| left == right)
}

pub(super) fn character_case_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char-not-equal", arguments, true)
}

pub(super) fn character_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<", arguments, false, |left, right| left < right)
}

pub(super) fn character_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>", arguments, false, |left, right| left > right)
}

pub(super) fn character_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<=", arguments, false, |left, right| left <= right)
}

pub(super) fn character_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>=", arguments, false, |left, right| left >= right)
}

pub(super) fn character_case_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-lessp", arguments, true, |left, right| left < right)
}

pub(super) fn character_case_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-greaterp", arguments, true, |left, right| left > right)
}

pub(super) fn character_case_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-greaterp", arguments, true, |left, right| {
        left <= right
    })
}

pub(super) fn character_case_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-lessp", arguments, true, |left, right| {
        left >= right
    })
}

pub(super) fn compare_characters(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
    comparison: fn(char, char) -> bool,
) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity(function, "at least 2", arguments.len()));
    }
    let characters = arguments
        .iter()
        .map(|value| character_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(characters.windows(2).all(|window| {
        let left = if ignore_case {
            window[0].to_ascii_lowercase()
        } else {
            window[0]
        };
        let right = if ignore_case {
            window[1].to_ascii_lowercase()
        } else {
            window[1]
        };
        comparison(left, right)
    })))
}

pub(super) fn compare_characters_distinct(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity(function, "at least 2", arguments.len()));
    }
    let characters = arguments
        .iter()
        .map(|value| character_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, left) in characters.iter().enumerate() {
        for right in characters.iter().skip(index + 1) {
            let left = if ignore_case {
                left.to_ascii_lowercase()
            } else {
                *left
            };
            let right = if ignore_case {
                right.to_ascii_lowercase()
            } else {
                *right
            };
            if left == right {
                return Ok(Value::Nil);
            }
        }
    }
    Ok(Value::boolean(true))
}

pub(super) fn character_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-upcase", 1)?;
    Ok(Value::Character(
        character_argument("char-upcase", &arguments[0])?.to_ascii_uppercase(),
    ))
}

pub(super) fn character_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-downcase", 1)?;
    Ok(Value::Character(
        character_argument("char-downcase", &arguments[0])?.to_ascii_lowercase(),
    ))
}

pub(super) fn alpha_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alpha-char-p", arguments, |character| {
        character.is_alphabetic()
    })
}

pub(super) fn alphanumeric_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alphanumericp", arguments, |character| {
        character.is_alphanumeric()
    })
}

pub(super) fn graphic_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("graphic-char-p", arguments, |character| {
        !character.is_control()
    })
}

pub(super) fn standard_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("standard-char-p", arguments, |character| {
        character == '\n' || character == ' ' || character.is_ascii_graphic()
    })
}

pub(super) fn upper_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("upper-case-p", arguments, char::is_uppercase)
}

pub(super) fn lower_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("lower-case-p", arguments, char::is_lowercase)
}

pub(super) fn both_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("both-case-p", arguments, |character| {
        character.is_uppercase() || character.is_lowercase()
    })
}

pub(super) fn character_predicate(
    function: &str,
    arguments: &[Value],
    predicate: impl Fn(char) -> bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 1)?;
    Ok(Value::boolean(predicate(character_argument(
        function,
        &arguments[0],
    )?)))
}

pub(super) fn digit_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("digit-char", "1 or 2", arguments.len()));
    }
    let weight = integer_argument("digit-char", &arguments[0])?;
    let radix = radix_argument("digit-char", arguments, 1)?;
    if weight < 0 || weight >= i64::from(radix) {
        return Ok(Value::Nil);
    }
    let Some(digit) = u32::try_from(weight).ok() else {
        return Ok(Value::Nil);
    };
    let character = if digit < 10 {
        let Some(digit) = u8::try_from(digit).ok() else {
            return Ok(Value::Nil);
        };
        char::from(b'0' + digit)
    } else {
        let Some(digit) = u8::try_from(digit - 10).ok() else {
            return Ok(Value::Nil);
        };
        char::from(b'A' + digit)
    };
    Ok(Value::Character(character))
}

pub(super) fn digit_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("digit-char-p", "1 or 2", arguments.len()));
    }
    let character = character_argument("digit-char-p", &arguments[0])?;
    let radix = radix_argument("digit-char-p", arguments, 1)?;
    let digit = match character {
        '0'..='9' => Some(u32::from(character) - u32::from('0')),
        'A'..='Z' => Some(u32::from(character) - u32::from('A') + 10),
        'a'..='z' => Some(u32::from(character) - u32::from('a') + 10),
        _ => None,
    };
    match digit {
        Some(digit) if digit < radix => Ok(Value::Integer(i64::from(digit))),
        _ => Ok(Value::Nil),
    }
}

pub(super) fn radix_argument(
    function: &str,
    arguments: &[Value],
    index: usize,
) -> Result<u32, RuntimeError> {
    let radix = arguments
        .get(index)
        .map(|value| integer_argument(function, value))
        .transpose()?
        .unwrap_or(10);
    if !(2..=36).contains(&radix) {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} radix must be between 2 and 36"),
            span: None,
        });
    }
    u32::try_from(radix).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} radix must be between 2 and 36"),
        span: None,
    })
}

pub(super) fn character_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-name", 1)?;
    Ok(
        named_character_name(character_argument("char-name", &arguments[0])?)
            .map_or(Value::Nil, Value::string),
    )
}

pub(super) fn name_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "name-char", 1)?;
    let name = string_designator("name-char", &arguments[0])?;
    if let Some(character) = named_character_from_name(&name) {
        return Ok(Value::Character(character));
    }
    let mut characters = name.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => Ok(Value::Character(character)),
        _ => Ok(Value::Nil),
    }
}

pub(super) const fn named_character_name(character: char) -> Option<&'static str> {
    match character {
        '\0' => Some("Null"),
        '\x07' => Some("Bell"),
        '\x08' => Some("Backspace"),
        '\t' => Some("Tab"),
        '\n' => Some("Newline"),
        '\x0c' => Some("Page"),
        '\r' => Some("Return"),
        ' ' => Some("Space"),
        '\x7f' => Some("Rubout"),
        _ => None,
    }
}

pub(super) fn named_character_from_name(name: &str) -> Option<char> {
    match name.to_ascii_uppercase().as_str() {
        "NULL" | "NUL" => Some('\0'),
        "BELL" => Some('\x07'),
        "BACKSPACE" | "BS" => Some('\x08'),
        "TAB" => Some('\t'),
        "NEWLINE" | "LINEFEED" | "LF" => Some('\n'),
        "PAGE" | "FORMFEED" | "FF" => Some('\x0c'),
        "RETURN" | "CR" => Some('\r'),
        "SPACE" => Some(' '),
        "RUBOUT" | "DELETE" | "DEL" => Some('\x7f'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type CharacterComparison = fn(&[Value]) -> Result<Value, RuntimeError>;

    fn cv(value: char) -> Value {
        Value::Character(value)
    }

    #[test]
    fn converts_characters_and_names() {
        let cases = [
            (character_value(&[Value::string("A")]), cv('A')),
            (character_value(&[cv('A')]), cv('A')),
            (
                make_string(&[Value::Integer(3), cv('x')]),
                Value::string("xxx"),
            ),
            (make_string(&[Value::Integer(2)]), Value::string("  ")),
            (
                character(&[Value::string("rust"), Value::Integer(2)]),
                cv('s'),
            ),
            (
                simple_character(&[Value::string("λ"), Value::Integer(0)]),
                cv('λ'),
            ),
            (code_char(&[Value::Integer(0x1f600)]), cv('😀')),
            (code_char(&[Value::Integer(-1)]), Value::Nil),
            (char_code(&[cv('A')]), Value::Integer(65)),
            (char_int(&[cv('A')]), Value::Integer(65)),
            (int_char(&[Value::Integer(65)]), cv('A')),
            (character_name(&[cv('\n')]), Value::string("Newline")),
            (name_character(&[Value::string("lf")]), cv('\n')),
            (name_character(&[Value::string("λ")]), cv('λ')),
            (name_character(&[Value::string("unknown")]), Value::Nil),
        ];
        for (actual, expected) in cases {
            assert_eq!(
                actual
                    .unwrap_or_else(|error| panic!("character conversion: {error}"))
                    .to_string(),
                expected.to_string()
            );
        }
        let named = [
            ('\0', "Null", &["null", "nul"] as &[&str]),
            ('\x07', "Bell", &["bell"]),
            ('\x08', "Backspace", &["backspace", "bs"]),
            ('\t', "Tab", &["tab"]),
            ('\n', "Newline", &["newline", "linefeed", "lf"]),
            ('\x0c', "Page", &["page", "formfeed", "ff"]),
            ('\r', "Return", &["return", "cr"]),
            (' ', "Space", &["space"]),
            ('\x7f', "Rubout", &["rubout", "delete", "del"]),
        ];
        for (character, name, aliases) in named {
            assert_eq!(named_character_name(character), Some(name));
            for alias in aliases {
                assert_eq!(named_character_from_name(alias), Some(character));
            }
        }
        assert_eq!(named_character_name('x'), None);
        assert_eq!(named_character_from_name("unknown"), None);
    }

    #[test]
    fn character_predicates_and_radix_operations_are_table_driven() {
        let cases = [
            (
                digit_character(&[Value::Integer(15), Value::Integer(16)]),
                cv('F'),
            ),
            (
                digit_character(&[Value::Integer(16), Value::Integer(16)]),
                Value::Nil,
            ),
            (
                digit_character_p(&[cv('f'), Value::Integer(16)]),
                Value::Integer(15),
            ),
            (
                digit_character_p(&[cv('f'), Value::Integer(15)]),
                Value::Nil,
            ),
            (
                digit_character_p(&[cv('!'), Value::Integer(16)]),
                Value::Nil,
            ),
            (alpha_character_p(&[cv('é')]), Value::boolean(true)),
            (alphanumeric_p(&[cv('7')]), Value::boolean(true)),
            (graphic_character_p(&[cv(' ')]), Value::boolean(true)),
            (standard_character_p(&[cv('\0')]), Value::boolean(false)),
            (upper_case_p(&[cv('A')]), Value::boolean(true)),
            (lower_case_p(&[cv('a')]), Value::boolean(true)),
            (both_case_p(&[cv('ß')]), Value::boolean(true)),
            (character_upcase(&[cv('é')]), cv('é')),
            (character_downcase(&[cv('É')]), cv('É')),
        ];
        for (actual, expected) in cases {
            assert_eq!(
                actual
                    .unwrap_or_else(|error| panic!("character predicate: {error}"))
                    .to_string(),
                expected.to_string()
            );
        }
        assert!(alpha_character_p(&[Value::Integer(1)]).is_err());
    }

    #[test]
    fn character_comparisons_cover_case_and_distinctness() {
        let cases = [
            (
                character_equal(&[cv('a'), cv('a'), cv('a')]),
                Value::boolean(true),
            ),
            (
                character_case_equal(&[cv('A'), cv('a')]),
                Value::boolean(true),
            ),
            (
                character_less_than(&[cv('a'), cv('b')]),
                Value::boolean(true),
            ),
            (
                character_greater_than(&[cv('b'), cv('a')]),
                Value::boolean(true),
            ),
            (
                character_less_equal(&[cv('a'), cv('a')]),
                Value::boolean(true),
            ),
            (
                character_greater_equal(&[cv('b'), cv('b')]),
                Value::boolean(true),
            ),
            (
                character_case_less_than(&[cv('A'), cv('b')]),
                Value::boolean(true),
            ),
            (
                character_case_greater_than(&[cv('B'), cv('a')]),
                Value::boolean(true),
            ),
            (
                character_case_less_equal(&[cv('A'), cv('a')]),
                Value::boolean(true),
            ),
            (
                character_case_greater_equal(&[cv('B'), cv('a')]),
                Value::boolean(true),
            ),
            (
                character_not_equal(&[cv('a'), cv('b'), cv('c')]),
                Value::boolean(true),
            ),
            (character_case_not_equal(&[cv('A'), cv('a')]), Value::Nil),
        ];
        for (actual, expected) in cases {
            assert_eq!(
                actual
                    .unwrap_or_else(|error| panic!("character comparison: {error}"))
                    .to_string(),
                expected.to_string()
            );
        }
        let unary_comparison_functions: [CharacterComparison; 8] = [
            character_equal,
            character_not_equal,
            character_less_than,
            character_greater_than,
            character_less_equal,
            character_greater_equal,
            character_case_equal,
            character_case_not_equal,
        ];
        for function in unary_comparison_functions {
            assert!(function(&[cv('a')]).is_err());
        }
    }
}

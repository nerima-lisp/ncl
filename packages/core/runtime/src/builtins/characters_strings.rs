use super::*;

pub(crate) fn string_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "string", 1)?;
    Ok(Value::string(string_designator("string", &arguments[0])?))
}

pub(crate) fn make_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(crate) fn character_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "character", 1)?;
    Ok(Value::Character(character_designator(
        "character",
        &arguments[0],
    )?))
}

pub(crate) fn character(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(crate) fn simple_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(crate) fn char_code(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-code", 1)?;
    Ok(Value::Integer(
        character_argument("char-code", &arguments[0])? as i64,
    ))
}

pub(crate) fn char_int(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-int", 1)?;
    Ok(Value::Integer(
        character_argument("char-int", &arguments[0])? as i64,
    ))
}

pub(crate) fn code_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "code-char", 1)?;
    code_char_value("code-char", &arguments[0])
}

pub(crate) fn int_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "int-char", 1)?;
    code_char_value("int-char", &arguments[0])
}

pub(crate) fn code_char_value(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let code = integer_argument(function, value)?;
    Ok(u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .map(Value::Character)
        .unwrap_or(Value::Nil))
}

pub(crate) fn character_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char=", arguments, false, |left, right| left == right)
}

pub(crate) fn character_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char/=", arguments, false)
}

pub(crate) fn character_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-equal", arguments, true, |left, right| left == right)
}

pub(crate) fn character_case_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char-not-equal", arguments, true)
}

pub(crate) fn character_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<", arguments, false, |left, right| left < right)
}

pub(crate) fn character_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>", arguments, false, |left, right| left > right)
}

pub(crate) fn character_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<=", arguments, false, |left, right| left <= right)
}

pub(crate) fn character_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>=", arguments, false, |left, right| left >= right)
}

pub(crate) fn character_case_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-lessp", arguments, true, |left, right| left < right)
}

pub(crate) fn character_case_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-greaterp", arguments, true, |left, right| left > right)
}

pub(crate) fn character_case_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-greaterp", arguments, true, |left, right| {
        left <= right
    })
}

pub(crate) fn character_case_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-lessp", arguments, true, |left, right| {
        left >= right
    })
}

pub(crate) fn compare_characters(
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

pub(crate) fn compare_characters_distinct(
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

pub(crate) fn character_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-upcase", 1)?;
    Ok(Value::Character(
        character_argument("char-upcase", &arguments[0])?.to_ascii_uppercase(),
    ))
}

pub(crate) fn character_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-downcase", 1)?;
    Ok(Value::Character(
        character_argument("char-downcase", &arguments[0])?.to_ascii_lowercase(),
    ))
}

pub(crate) fn alpha_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alpha-char-p", arguments, |character| {
        character.is_alphabetic()
    })
}

pub(crate) fn alphanumeric_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alphanumericp", arguments, |character| {
        character.is_alphanumeric()
    })
}

pub(crate) fn graphic_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("graphic-char-p", arguments, |character| {
        !character.is_control()
    })
}

pub(crate) fn standard_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("standard-char-p", arguments, |character| {
        character == '\n' || character == ' ' || character.is_ascii_graphic()
    })
}

pub(crate) fn upper_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("upper-case-p", arguments, char::is_uppercase)
}

pub(crate) fn lower_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("lower-case-p", arguments, char::is_lowercase)
}

pub(crate) fn both_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("both-case-p", arguments, |character| {
        character.is_uppercase() || character.is_lowercase()
    })
}

pub(crate) fn character_predicate(
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

pub(crate) fn digit_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("digit-char", "1 or 2", arguments.len()));
    }
    let weight = integer_argument("digit-char", &arguments[0])?;
    let radix = radix_argument("digit-char", arguments, 1)?;
    if weight < 0 || weight >= i64::from(radix) {
        return Ok(Value::Nil);
    }
    let digit = weight as u32;
    let character = if digit < 10 {
        (b'0' + digit as u8) as char
    } else {
        (b'A' + (digit - 10) as u8) as char
    };
    Ok(Value::Character(character))
}

pub(crate) fn digit_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("digit-char-p", "1 or 2", arguments.len()));
    }
    let character = character_argument("digit-char-p", &arguments[0])?;
    let radix = radix_argument("digit-char-p", arguments, 1)?;
    let digit = match character {
        '0'..='9' => Some(character as u32 - '0' as u32),
        'A'..='Z' => Some(character as u32 - 'A' as u32 + 10),
        'a'..='z' => Some(character as u32 - 'a' as u32 + 10),
        _ => None,
    };
    match digit {
        Some(digit) if digit < radix => Ok(Value::Integer(i64::from(digit))),
        _ => Ok(Value::Nil),
    }
}

pub(crate) fn radix_argument(
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
    Ok(radix as u32)
}

pub(crate) fn character_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-name", 1)?;
    Ok(
        named_character_name(character_argument("char-name", &arguments[0])?)
            .map(Value::string)
            .unwrap_or(Value::Nil),
    )
}

pub(crate) fn name_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(crate) fn named_character_name(character: char) -> Option<&'static str> {
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

pub(crate) fn named_character_from_name(name: &str) -> Option<char> {
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

pub(crate) fn string_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string=", arguments, false)
}

pub(crate) fn string_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string-equal", arguments, true)
}

pub(crate) fn string_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<", arguments, false, |ordering| {
        ordering == Ordering::Less
    })
}

pub(crate) fn string_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>", arguments, false, |ordering| {
        ordering == Ordering::Greater
    })
}

pub(crate) fn string_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<=", arguments, false, |ordering| {
        ordering != Ordering::Greater
    })
}

pub(crate) fn string_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>=", arguments, false, |ordering| {
        ordering != Ordering::Less
    })
}

pub(crate) fn compare_strings(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
    comparison: fn(Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = string_designator(function, &arguments[0])?;
    let right = string_designator(function, &arguments[1])?;
    let (index, ordering) = string_order(&left, &right, ignore_case);
    if comparison(ordering) {
        Ok(Value::Integer(index as i64))
    } else {
        Ok(Value::Nil)
    }
}

pub(crate) fn string_equality(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = string_designator(function, &arguments[0])?;
    let right = string_designator(function, &arguments[1])?;
    let (_, ordering) = string_order(&left, &right, ignore_case);
    Ok(Value::boolean(ordering == Ordering::Equal))
}

pub(crate) fn string_order(left: &str, right: &str, ignore_case: bool) -> (usize, Ordering) {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    for (index, (left, right)) in left.iter().zip(&right).enumerate() {
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
        if left != right {
            return (index, left.cmp(&right));
        }
    }
    (left.len().min(right.len()), left.len().cmp(&right.len()))
}

pub(crate) fn string_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-upcase", StringCase::Upper)
}

pub(crate) fn string_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-downcase", StringCase::Lower)
}

pub(crate) fn string_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-capitalize", StringCase::Capitalize)
}

pub(crate) fn nstring_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-upcase", StringCase::Upper)
}

pub(crate) fn nstring_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-downcase", StringCase::Lower)
}

pub(crate) fn nstring_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-capitalize", StringCase::Capitalize)
}

#[derive(Clone, Copy)]
pub(crate) enum StringCase {
    Upper,
    Lower,
    Capitalize,
}

pub(crate) fn string_case_transform(
    arguments: &[Value],
    function: &str,
    case: StringCase,
) -> Result<Value, RuntimeError> {
    if !(1..=5).contains(&arguments.len()) || !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(function, "1, 3, or 5", arguments.len()));
    }
    let value = string_designator(function, &arguments[0])?;
    let characters = value.chars().collect::<Vec<_>>();
    let (start, end) = sequence_bounds(function, characters.len(), &arguments[1..])?;
    let mut output = String::new();
    let mut word_start = true;
    for (index, character) in characters.into_iter().enumerate() {
        let in_range = (start..end).contains(&index);
        match case {
            StringCase::Upper if in_range => output.extend(character.to_uppercase()),
            StringCase::Lower if in_range => output.extend(character.to_lowercase()),
            StringCase::Capitalize if character.is_alphanumeric() => {
                if in_range && word_start {
                    output.extend(character.to_uppercase());
                } else if in_range {
                    output.extend(character.to_lowercase());
                } else {
                    output.push(character);
                }
                word_start = false;
            }
            StringCase::Capitalize => {
                output.push(character);
                word_start = true;
            }
            _ => output.push(character),
        }
    }
    Ok(Value::string(output))
}

pub(crate) fn string_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-trim", true, true)
}

pub(crate) fn string_left_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-left-trim", true, false)
}

pub(crate) fn string_right_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-right-trim", false, true)
}

pub(crate) fn trim_string(
    arguments: &[Value],
    function: &str,
    trim_left: bool,
    trim_right: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let trim_set = sequence_elements(function, &arguments[0])?
        .into_iter()
        .map(|value| character_argument(function, &value))
        .collect::<Result<Vec<_>, _>>()?;
    let value = string_designator(function, &arguments[1])?;
    let characters = value.chars().collect::<Vec<_>>();
    let is_trimmed = |character: &char| trim_set.contains(character);
    let start = if trim_left {
        characters
            .iter()
            .position(|character| !is_trimmed(character))
    } else {
        Some(0)
    }
    .unwrap_or(characters.len());
    let end = if trim_right {
        characters
            .iter()
            .rposition(|character| !is_trimmed(character))
            .map_or(0, |index| index + 1)
    } else {
        characters.len()
    };
    Ok(Value::string(
        characters[start.min(end)..end].iter().collect::<String>(),
    ))
}

pub(crate) fn character_argument(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        value => Err(type_error(function, "character", value)),
    }
}

pub(crate) fn character_designator(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        Value::String(_)
        | Value::Symbol(_)
        | Value::UninternedSymbol(_)
        | Value::Keyword(_)
        | Value::SymbolExact(_)
        | Value::KeywordExact(_) => {
            let string = string_designator(function, value)?;
            let mut characters = string.chars();
            match (characters.next(), characters.next()) {
                (Some(character), None) => Ok(character),
                _ => Err(type_error(function, "character designator", value)),
            }
        }
        value => Err(type_error(function, "character designator", value)),
    }
}

pub(crate) fn string_designator(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Nil => Ok("NIL".to_string()),
        Value::Boolean(true) => Ok("T".to_string()),
        Value::Boolean(false) => Ok("NIL".to_string()),
        Value::String(value)
        | Value::Symbol(value)
        | Value::UninternedSymbol(value)
        | Value::Keyword(value)
        | Value::SymbolExact(value)
        | Value::KeywordExact(value) => Ok(value.to_string()),
        Value::Character(value) => Ok(value.to_string()),
        value => Err(type_error(function, "string designator", value)),
    }
}

macro_rules! character_builtins {
    () => {
fn string_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "string", 1)?;
    Ok(Value::string(string_designator("string", &arguments[0])?))
}

fn validate_make_string_element_type(value: &Value) -> Result<(), RuntimeError> {
    let element_type = type_designator_name("make-string", value)?;
    match element_type.as_str() {
        "CHARACTER" | "BASE-CHAR" | "STANDARD-CHAR" | "EXTENDED-CHAR" => Ok(()),
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "make-string :element-type must be a character type, got {element_type}"
            ),
            span: None,
        }),
    }
}

fn make_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-string", "at least 1", 0));
    }
    let length = index_argument("make-string", &arguments[0])?;
    let mut initial = ' ';
    match arguments.get(1) {
        None => {}
        Some(value)
            if arguments.len() == 2
                && !matches!(value, Value::Keyword(_) | Value::KeywordExact(_)) =>
        {
            initial = character_argument("make-string", value)?;
        }
        Some(_) => {
            if !(arguments.len() - 1).is_multiple_of(2) {
                return Err(arity(
                    "make-string",
                    "a size and keyword/value pairs",
                    arguments.len(),
                ));
            }
            for pair in arguments[1..].chunks_exact(2) {
                match array_option_name("make-string", &pair[0])?.as_str() {
                    "INITIAL-ELEMENT" => {
                        initial = character_argument("make-string", &pair[1])?;
                    }
                    "ELEMENT-TYPE" => validate_make_string_element_type(&pair[1])?,
                    option => {
                        return Err(RuntimeError::InvalidForm {
                            message: format!("make-string does not accept :{option}"),
                            span: None,
                        });
                    }
                }
            }
        }
    }
    Ok(Value::string(
        std::iter::repeat_n(initial, length).collect::<String>(),
    ))
}

fn character_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "character", 1)?;
    Ok(Value::Character(character_designator(
        "character",
        &arguments[0],
    )?))
}

fn character(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn simple_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn char_code(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-code", 1)?;
    Ok(Value::Integer(
        character_argument("char-code", &arguments[0])? as i64,
    ))
}

fn char_int(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-int", 1)?;
    Ok(Value::Integer(
        character_argument("char-int", &arguments[0])? as i64,
    ))
}

fn code_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "code-char", 1)?;
    code_char_value("code-char", &arguments[0])
}

fn int_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "int-char", 1)?;
    code_char_value("int-char", &arguments[0])
}

fn code_char_value(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let code = integer_argument(function, value)?;
    Ok(u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .map(Value::Character)
        .unwrap_or(Value::Nil))
}

fn character_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char=", arguments, false, |left, right| left == right)
}

fn character_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char/=", arguments, false)
}

fn character_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-equal", arguments, true, |left, right| left == right)
}

fn character_case_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char-not-equal", arguments, true)
}

fn character_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<", arguments, false, |left, right| left < right)
}

fn character_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>", arguments, false, |left, right| left > right)
}

fn character_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<=", arguments, false, |left, right| left <= right)
}

fn character_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>=", arguments, false, |left, right| left >= right)
}

fn character_case_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-lessp", arguments, true, |left, right| left < right)
}

fn character_case_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-greaterp", arguments, true, |left, right| left > right)
}

fn character_case_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-greaterp", arguments, true, |left, right| {
        left <= right
    })
}

fn character_case_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-lessp", arguments, true, |left, right| {
        left >= right
    })
}

fn compare_characters(
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

fn compare_characters_distinct(
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

fn character_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-upcase", 1)?;
    Ok(Value::Character(
        character_argument("char-upcase", &arguments[0])?.to_ascii_uppercase(),
    ))
}

fn character_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-downcase", 1)?;
    Ok(Value::Character(
        character_argument("char-downcase", &arguments[0])?.to_ascii_lowercase(),
    ))
}

fn alpha_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alpha-char-p", arguments, |character| {
        character.is_alphabetic()
    })
}

fn alphanumeric_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alphanumericp", arguments, |character| {
        character.is_alphanumeric()
    })
}

fn graphic_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("graphic-char-p", arguments, |character| {
        !character.is_control()
    })
}

fn standard_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("standard-char-p", arguments, |character| {
        character == '\n' || character == ' ' || character.is_ascii_graphic()
    })
}

fn upper_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("upper-case-p", arguments, char::is_uppercase)
}

fn lower_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("lower-case-p", arguments, char::is_lowercase)
}

fn both_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("both-case-p", arguments, |character| {
        character.is_uppercase() || character.is_lowercase()
    })
}

fn character_predicate(
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

fn digit_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn digit_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn radix_argument(function: &str, arguments: &[Value], index: usize) -> Result<u32, RuntimeError> {
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

fn character_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-name", 1)?;
    Ok(
        named_character_name(character_argument("char-name", &arguments[0])?)
            .map(Value::string)
            .unwrap_or(Value::Nil),
    )
}

fn name_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn named_character_name(character: char) -> Option<&'static str> {
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

fn named_character_from_name(name: &str) -> Option<char> {
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


    };
}

use super::*;

pub(crate) fn parse_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "parse-integer",
            "a string and keyword/value pairs",
            arguments.len(),
        ));
    }
    let chars = match &arguments[0] {
        Value::String(value) => value.as_ref().chars().collect::<Vec<_>>(),
        value => return Err(type_error("parse-integer", "a string", value)),
    };
    let mut start = 0;
    let mut end = chars.len();
    let mut radix = 10_i64;
    let mut junk_allowed = false;
    for pair in arguments[1..].chunks_exact(2) {
        match array_option_name("parse-integer", &pair[0])?.as_str() {
            "START" => start = index_argument("parse-integer", &pair[1])?,
            "END" => end = index_argument("parse-integer", &pair[1])?,
            "RADIX" => radix = integer_argument("parse-integer", &pair[1])?,
            "JUNK-ALLOWED" => junk_allowed = pair[1].is_truthy(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("parse-integer does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > chars.len() {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer bounds are invalid".to_string(),
            span: None,
        });
    }
    if !(2..=36).contains(&radix) {
        return Err(RuntimeError::InvalidForm {
            message: format!("parse-integer radix must be between 2 and 36, got {radix}"),
            span: None,
        });
    }
    let radix = u32::try_from(radix).expect("parse-integer radix was checked");
    let mut cursor = start;
    while cursor < end && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    let negative = match chars.get(cursor) {
        Some('+') => {
            cursor += 1;
            false
        }
        Some('-') => {
            cursor += 1;
            true
        }
        _ => false,
    };
    let digits_start = cursor;
    let mut magnitude = 0_i128;
    while cursor < end {
        let Some(digit) = parse_integer_digit(chars[cursor]) else {
            break;
        };
        if digit >= radix {
            break;
        }
        magnitude = magnitude
            .checked_mul(i128::from(radix))
            .and_then(|value| value.checked_add(i128::from(digit)))
            .ok_or(RuntimeError::NumericOverflow)?;
        cursor += 1;
    }
    if cursor == digits_start {
        if junk_allowed {
            let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
            return Ok(Value::values(vec![Value::Nil, Value::Integer(position)]));
        }
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found no integer".to_string(),
            span: None,
        });
    }
    let signed = if negative {
        magnitude
            .checked_neg()
            .ok_or(RuntimeError::NumericOverflow)?
    } else {
        magnitude
    };
    let integer = i64::try_from(signed).map_err(|_| RuntimeError::NumericOverflow)?;
    if junk_allowed {
        let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
        return Ok(Value::values(vec![
            Value::Integer(integer),
            Value::Integer(position),
        ]));
    }
    let mut trailing = cursor;
    while trailing < end && chars[trailing].is_whitespace() {
        trailing += 1;
    }
    if trailing != end {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found junk after the integer".to_string(),
            span: None,
        });
    }
    let position = i64::try_from(end).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![
        Value::Integer(integer),
        Value::Integer(position),
    ]))
}

pub(crate) fn parse_integer_digit(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(u32::from(character as u8 - b'0')),
        'A'..='Z' => Some(u32::from(character as u8 - b'A') + 10),
        'a'..='z' => Some(u32::from(character as u8 - b'a') + 10),
        _ => None,
    }
}

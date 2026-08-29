use super::options::ParseIntegerOptions;
use crate::{RuntimeError, Value};

pub(super) fn parse_integer_value(
    chars: &[char],
    options: ParseIntegerOptions,
) -> Result<Value, RuntimeError> {
    let ParseIntegerOptions {
        start,
        end,
        radix,
        junk_allowed,
    } = options;
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
            return parse_integer_result(None, cursor);
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
        return parse_integer_result(Some(integer), cursor);
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
    parse_integer_result(Some(integer), end)
}

fn parse_integer_result(integer: Option<i64>, position: usize) -> Result<Value, RuntimeError> {
    let position = i64::try_from(position).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![
        integer.map_or(Value::Nil, Value::Integer),
        Value::Integer(position),
    ]))
}

pub(super) fn parse_integer_digit(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(u32::from(character as u8 - b'0')),
        'A'..='Z' => Some(u32::from(character as u8 - b'A') + 10),
        'a'..='z' => Some(u32::from(character as u8 - b'a') + 10),
        _ => None,
    }
}

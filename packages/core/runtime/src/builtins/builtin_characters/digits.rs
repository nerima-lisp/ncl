use super::{RuntimeError, Value, arity, character_argument, integer_argument};

pub fn digit_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub fn digit_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn radix_argument(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_string(result: Result<Value, RuntimeError>) -> String {
        match result {
            Ok(value) => value.to_string(),
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn converts_digits_to_and_from_characters() {
        let cases = [
            (
                digit_character(&[Value::Integer(15), Value::Integer(16)]),
                Value::Character('F'),
            ),
            (
                digit_character(&[Value::Integer(16), Value::Integer(16)]),
                Value::Nil,
            ),
            (
                digit_character_p(&[Value::Character('f'), Value::Integer(16)]),
                Value::Integer(15),
            ),
            (
                digit_character_p(&[Value::Character('f'), Value::Integer(15)]),
                Value::Nil,
            ),
            (
                digit_character_p(&[Value::Character('!'), Value::Integer(16)]),
                Value::Nil,
            ),
        ];
        for (actual, expected) in cases {
            assert_eq!(ok_string(actual), expected.to_string());
        }
        assert!(digit_character(&[]).is_err());
    }
}

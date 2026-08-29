use super::{
    RuntimeError, Value, character_argument, character_designator, exact, integer_argument,
};

pub fn character_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "character", 1)?;
    Ok(Value::Character(character_designator(
        "character",
        &arguments[0],
    )?))
}

pub fn char_code(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-code", 1)?;
    Ok(Value::Integer(
        character_argument("char-code", &arguments[0])? as i64,
    ))
}

pub fn char_int(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-int", 1)?;
    Ok(Value::Integer(
        character_argument("char-int", &arguments[0])? as i64,
    ))
}

pub fn code_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "code-char", 1)?;
    code_char_value("code-char", &arguments[0])
}

pub fn int_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "int-char", 1)?;
    code_char_value("int-char", &arguments[0])
}

fn code_char_value(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let code = integer_argument(function, value)?;
    Ok(u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .map_or(Value::Nil, Value::Character))
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
    fn converts_between_characters_codes_and_ints() {
        let cases = [
            (character_value(&[Value::string("A")]), Value::Character('A')),
            (character_value(&[Value::Character('A')]), Value::Character('A')),
            (code_char(&[Value::Integer(0x1f600)]), Value::Character('😀')),
            (char_code(&[Value::Character('A')]), Value::Integer(65)),
            (char_int(&[Value::Character('A')]), Value::Integer(65)),
            (int_char(&[Value::Integer(65)]), Value::Character('A')),
            (code_char(&[Value::Integer(-1)]), Value::Nil),
        ];
        for (actual, expected) in cases {
            assert_eq!(ok_string(actual), expected.to_string());
        }
        assert!(character_value(&[Value::Integer(1)]).is_err());
    }
}

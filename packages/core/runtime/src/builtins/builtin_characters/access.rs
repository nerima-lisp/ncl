use super::{
    RuntimeError, Value, arity, character_argument, exact, index_argument, out_of_bounds,
    string_designator, type_error,
};

pub fn string_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "string", 1)?;
    Ok(Value::string(string_designator("string", &arguments[0])?))
}

pub fn make_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("make-string", "1 or 2", arguments.len()));
    }
    let length = index_argument("make-string", &arguments[0])?;
    let initial = arguments
        .get(1)
        .map(|value| character_argument("make-string", value))
        .transpose()?
        .unwrap_or(' ');
    Ok(Value::mutable_string(
        std::iter::repeat_n(initial, length).collect::<String>(),
    ))
}

pub fn character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char", 2)?;
    let index = index_argument("char", &arguments[1])?;
    let Some(value) = arguments[0].string_contents() else {
        return Err(type_error("char", "string", &arguments[0]));
    };
    value
        .chars()
        .nth(index)
        .map(Value::Character)
        .ok_or_else(|| out_of_bounds("char", index))
}

pub fn simple_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "schar", 2)?;
    let index = index_argument("schar", &arguments[1])?;
    let Some(value) = arguments[0].string_contents() else {
        return Err(type_error("schar", "simple-string", &arguments[0]));
    };
    value
        .chars()
        .nth(index)
        .map(Value::Character)
        .ok_or_else(|| out_of_bounds("schar", index))
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
    fn constructs_strings_and_accesses_characters() {
        assert_eq!(
            ok_string(string_value(&[Value::string("hi")])),
            Value::string("hi").to_string()
        );
        assert_eq!(
            ok_string(make_string(&[Value::Integer(3), Value::Character('x')])),
            Value::string("xxx").to_string()
        );
        assert_eq!(
            ok_string(make_string(&[Value::Integer(2)])),
            Value::string("  ").to_string()
        );
        assert_eq!(
            ok_string(character(&[Value::string("rust"), Value::Integer(2)])),
            Value::Character('s').to_string()
        );
        assert_eq!(
            ok_string(simple_character(&[Value::string("λ"), Value::Integer(0)])),
            Value::Character('λ').to_string()
        );
        assert!(character(&[Value::string("rust"), Value::Integer(9)]).is_err());
        assert!(simple_character(&[Value::Integer(1), Value::Integer(0)]).is_err());
    }

    #[test]
    fn make_string_rejects_wrong_argument_counts() {
        assert!(matches!(
            make_string(&[]),
            Err(RuntimeError::Arity { function, expected, actual })
                if function == "make-string" && expected == "1 or 2" && actual == 0
        ));
        assert!(matches!(
            make_string(&[Value::Integer(1), Value::Character('x'), Value::Integer(2)]),
            Err(RuntimeError::Arity { function, .. }) if function == "make-string"
        ));
    }
}

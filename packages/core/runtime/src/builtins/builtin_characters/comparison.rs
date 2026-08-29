use super::{RuntimeError, Value, arity, character_argument};

pub fn character_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char=", arguments, false, |left, right| left == right)
}

pub fn character_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char/=", arguments, false)
}

pub fn character_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<", arguments, false, |left, right| left < right)
}

pub fn character_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>", arguments, false, |left, right| left > right)
}

pub fn character_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<=", arguments, false, |left, right| left <= right)
}

pub fn character_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>=", arguments, false, |left, right| left >= right)
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
    fn case_sensitive_comparisons_and_distinctness() {
        let cv = Value::Character;
        let cases = [
            (character_equal(&[cv('a'), cv('a'), cv('a')]), true),
            (character_less_than(&[cv('a'), cv('b')]), true),
            (character_greater_than(&[cv('b'), cv('a')]), true),
            (character_less_equal(&[cv('a'), cv('a')]), true),
            (character_greater_equal(&[cv('b'), cv('b')]), true),
            (character_not_equal(&[cv('a'), cv('b'), cv('c')]), true),
        ];
        for (actual, expected) in cases {
            assert_eq!(ok_string(actual), Value::boolean(expected).to_string());
        }
        assert_eq!(
            ok_string(character_not_equal(&[cv('a'), cv('a')])),
            Value::Nil.to_string()
        );
        let unary_functions: [fn(&[Value]) -> Result<Value, RuntimeError>; 6] = [
            character_equal,
            character_not_equal,
            character_less_than,
            character_greater_than,
            character_less_equal,
            character_greater_equal,
        ];
        for function in unary_functions {
            assert!(function(&[Value::Character('a')]).is_err());
        }
    }
}

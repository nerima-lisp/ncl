use super::comparison::{compare_characters, compare_characters_distinct};
use super::{RuntimeError, Value};

pub fn character_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-equal", arguments, true, |left, right| left == right)
}

pub fn character_case_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char-not-equal", arguments, true)
}

pub fn character_case_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-lessp", arguments, true, |left, right| left < right)
}

pub fn character_case_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-greaterp", arguments, true, |left, right| left > right)
}

pub fn character_case_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-greaterp", arguments, true, |left, right| {
        left <= right
    })
}

pub fn character_case_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-lessp", arguments, true, |left, right| {
        left >= right
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
    fn case_insensitive_comparisons_and_distinctness() {
        let cv = Value::Character;
        let cases = [
            (character_case_equal(&[cv('A'), cv('a')]), true),
            (character_case_less_than(&[cv('A'), cv('b')]), true),
            (character_case_greater_than(&[cv('B'), cv('a')]), true),
            (character_case_less_equal(&[cv('A'), cv('a')]), true),
            (character_case_greater_equal(&[cv('B'), cv('a')]), true),
        ];
        for (actual, expected) in cases {
            assert_eq!(ok_string(actual), Value::boolean(expected).to_string());
        }
        assert_eq!(
            ok_string(character_case_not_equal(&[cv('A'), cv('a')])),
            Value::Nil.to_string()
        );
        let unary_functions: [fn(&[Value]) -> Result<Value, RuntimeError>; 6] = [
            character_case_equal,
            character_case_not_equal,
            character_case_less_than,
            character_case_greater_than,
            character_case_less_equal,
            character_case_greater_equal,
        ];
        for function in unary_functions {
            assert!(function(&[Value::Character('a')]).is_err());
        }
    }
}

use super::predicates::character_predicate;
use super::{RuntimeError, Value, character_argument, exact};

pub fn character_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-upcase", 1)?;
    Ok(Value::Character(
        character_argument("char-upcase", &arguments[0])?.to_ascii_uppercase(),
    ))
}

pub fn character_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-downcase", 1)?;
    Ok(Value::Character(
        character_argument("char-downcase", &arguments[0])?.to_ascii_lowercase(),
    ))
}

pub fn upper_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("upper-case-p", arguments, char::is_uppercase)
}

pub fn lower_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("lower-case-p", arguments, char::is_lowercase)
}

pub fn both_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("both-case-p", arguments, |character| {
        character.is_uppercase() || character.is_lowercase()
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
    fn converts_and_tests_character_case() {
        let cv = Value::Character;
        let cases = [
            (character_upcase(&[cv('é')]), cv('é')),
            (character_downcase(&[cv('É')]), cv('É')),
        ];
        for (actual, expected) in cases {
            assert_eq!(ok_string(actual), expected.to_string());
        }
        assert_eq!(
            ok_string(upper_case_p(&[cv('A')])),
            Value::boolean(true).to_string()
        );
        assert_eq!(
            ok_string(lower_case_p(&[cv('a')])),
            Value::boolean(true).to_string()
        );
        assert_eq!(
            ok_string(both_case_p(&[cv('ß')])),
            Value::boolean(true).to_string()
        );
        assert!(upper_case_p(&[Value::Integer(1)]).is_err());
    }
}

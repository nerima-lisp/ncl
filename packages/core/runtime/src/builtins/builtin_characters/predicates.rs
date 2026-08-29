use super::{RuntimeError, Value, character_argument, exact};

pub fn alpha_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alpha-char-p", arguments, |character| {
        character.is_alphabetic()
    })
}

pub fn alphanumeric_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alphanumericp", arguments, |character| {
        character.is_alphanumeric()
    })
}

pub fn graphic_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("graphic-char-p", arguments, |character| {
        !character.is_control()
    })
}

pub fn standard_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("standard-char-p", arguments, |character| {
        character == '\n' || character == ' ' || character.is_ascii_graphic()
    })
}

pub(super) fn character_predicate(
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
    fn classifies_characters_by_category() {
        let cv = Value::Character;
        let cases = [
            (alpha_character_p(&[cv('é')]), true),
            (alphanumeric_p(&[cv('7')]), true),
            (graphic_character_p(&[cv(' ')]), true),
            (standard_character_p(&[cv('\0')]), false),
        ];
        for (actual, expected) in cases {
            assert_eq!(ok_string(actual), Value::boolean(expected).to_string());
        }
        assert!(alpha_character_p(&[Value::Integer(1)]).is_err());
    }
}

use super::type_error;
use crate::{RuntimeError, Value};

pub fn character_argument(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        value => Err(type_error(function, "character", value)),
    }
}

pub fn character_designator(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        Value::String(_)
        | Value::Symbol(_)
        | Value::UninternedSymbol(_)
        | Value::Keyword(_)
        | Value::SymbolExact(_)
        | Value::KeywordExact(_) => {
            let string = string_designator(function, value)?;
            let mut characters = string.chars();
            match (characters.next(), characters.next()) {
                (Some(character), None) => Ok(character),
                _ => Err(type_error(function, "character designator", value)),
            }
        }
        value => Err(type_error(function, "character designator", value)),
    }
}

pub fn string_designator(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Nil | Value::Boolean(false) => Ok("NIL".to_string()),
        Value::Boolean(true) => Ok("T".to_string()),
        Value::String(value)
        | Value::Symbol(value)
        | Value::UninternedSymbol(value)
        | Value::Keyword(value)
        | Value::SymbolExact(value)
        | Value::KeywordExact(value) => Ok(value.to_string()),
        Value::Character(value) => Ok(value.to_string()),
        value => Err(type_error(function, "string designator", value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_string(result: Result<String, RuntimeError>) -> String {
        match result {
            Ok(value) => value,
            Err(error) => panic!("expected Ok, got {error:?}"),
        }
    }

    #[test]
    fn character_designator_rejects_multi_character_strings() {
        assert!(matches!(
            character_designator("test", &Value::string("ab")),
            Err(RuntimeError::Type { .. })
        ));
    }

    #[test]
    fn string_designator_normalizes_booleans_and_symbol_kinds() {
        assert_eq!(ok_string(string_designator("test", &Value::Nil)), "NIL");
        assert_eq!(
            ok_string(string_designator("test", &Value::Boolean(false))),
            "NIL"
        );
        assert_eq!(
            ok_string(string_designator("test", &Value::Boolean(true))),
            "T"
        );
        assert_eq!(
            ok_string(string_designator("test", &Value::uninterned_symbol("x"))),
            "x"
        );
        assert_eq!(
            ok_string(string_designator("test", &Value::keyword("x"))),
            "X"
        );
        assert_eq!(
            ok_string(string_designator("test", &Value::symbol_exact("x"))),
            "x"
        );
        assert_eq!(
            ok_string(string_designator("test", &Value::keyword_exact("x"))),
            "x"
        );
    }
}

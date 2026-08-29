use super::{arity, sequence_bounds, string_designator};
use crate::{RuntimeError, Value};

pub fn string_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-upcase", StringCase::Upper)
}

pub fn string_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-downcase", StringCase::Lower)
}

pub fn string_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-capitalize", StringCase::Capitalize)
}

pub fn nstring_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-upcase", StringCase::Upper)
}

pub fn nstring_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-downcase", StringCase::Lower)
}

pub fn nstring_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-capitalize", StringCase::Capitalize)
}

#[derive(Clone, Copy)]
pub(super) enum StringCase {
    Upper,
    Lower,
    Capitalize,
}

pub(in crate::builtins::builtin_sequences) fn string_case_transform(
    arguments: &[Value],
    function: &str,
    case: StringCase,
) -> Result<Value, RuntimeError> {
    if !(1..=5).contains(&arguments.len()) || !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(function, "1, 3, or 5", arguments.len()));
    }
    let value = string_designator(function, &arguments[0])?;
    let characters = value.chars().collect::<Vec<_>>();
    let (start, end) = sequence_bounds(function, characters.len(), &arguments[1..])?;
    let mut output = String::new();
    let mut word_start = true;
    for (index, character) in characters.into_iter().enumerate() {
        let in_range = (start..end).contains(&index);
        match case {
            StringCase::Upper if in_range => output.extend(character.to_uppercase()),
            StringCase::Lower if in_range => output.extend(character.to_lowercase()),
            StringCase::Capitalize if character.is_alphanumeric() => {
                if in_range && word_start {
                    output.extend(character.to_uppercase());
                } else if in_range {
                    output.extend(character.to_lowercase());
                } else {
                    output.push(character);
                }
                word_start = false;
            }
            StringCase::Capitalize => {
                output.push(character);
                word_start = true;
            }
            _ => output.push(character),
        }
    }
    Ok(Value::string(output))
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
    fn nstring_upcase_and_downcase_transform_full_strings() {
        assert_eq!(
            ok_string(nstring_upcase(&[Value::string("abc")])),
            Value::string("ABC").to_string()
        );
        assert_eq!(
            ok_string(nstring_downcase(&[Value::string("ABC")])),
            Value::string("abc").to_string()
        );
    }

    #[test]
    fn string_upcase_reports_an_arity_error() {
        assert!(matches!(
            string_upcase(&[]),
            Err(RuntimeError::Arity { .. })
        ));
    }

    #[test]
    fn string_capitalize_leaves_alphanumerics_outside_the_bounds_untouched() {
        let result = ok_string(string_capitalize(&[
            Value::string("abc def"),
            Value::keyword("start"),
            Value::Integer(4),
        ]));
        assert_eq!(result, Value::string("abc Def").to_string());
    }
}

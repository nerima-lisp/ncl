use super::{
    arity, array_option_name, character_argument, exact, index_argument, rebuild_sequence,
    sequence_elements, string_designator, type_designator_name, type_error,
};
use crate::{RuntimeError, Value};

pub fn copy_seq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-seq", 1)?;
    let items = sequence_elements("copy-seq", &arguments[0])?;
    rebuild_sequence("copy-seq", &arguments[0], items)
}

pub fn concatenate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("concatenate", "at least one", 0));
    }
    let result_type = type_designator_name("concatenate", &arguments[0])?;
    let mut items = Vec::new();
    for sequence in &arguments[1..] {
        items.extend(sequence_elements("concatenate", sequence)?);
    }
    match result_type.as_str() {
        "LIST" => Ok(Value::list(items)),
        "VECTOR" => Ok(Value::vector(items)),
        "STRING" | "SIMPLE-STRING" => {
            let mut result = String::new();
            for item in items {
                let Value::Character(character) = item else {
                    return Err(type_error(
                        "concatenate",
                        "characters for a string result",
                        &item,
                    ));
                };
                result.push(character);
            }
            Ok(Value::string(result))
        }
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "concatenate result type must be LIST, VECTOR, or STRING, got {result_type}"
            ),
            span: None,
        }),
    }
}

pub fn make_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 || !(arguments.len() - 2).is_multiple_of(2) {
        return Err(arity(
            "make-sequence",
            "a result type, a size, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let result_type = type_designator_name("make-sequence", &arguments[0])?;
    let size = index_argument("make-sequence", &arguments[1])?;
    let mut initial_element = Value::Nil;
    for pair in arguments[2..].as_chunks::<2>().0 {
        match array_option_name("make-sequence", &pair[0])?.as_str() {
            "INITIAL-ELEMENT" => initial_element = pair[1].clone(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-sequence does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    match result_type.as_str() {
        "LIST" => Ok(Value::list(vec![initial_element; size])),
        "VECTOR" | "SIMPLE-VECTOR" => Ok(Value::vector(vec![initial_element; size])),
        "STRING" | "SIMPLE-STRING" => {
            let initial = character_argument("make-sequence", &initial_element)?;
            Ok(Value::string(
                std::iter::repeat_n(initial, size).collect::<String>(),
            ))
        }
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "make-sequence result type must be LIST, VECTOR, or STRING, got {result_type}"
            ),
            span: None,
        }),
    }
}

pub fn coerce(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "coerce", 2)?;
    let result_type = type_designator_name("coerce", &arguments[1])?;
    match result_type.as_str() {
        "LIST" => Ok(Value::list(sequence_elements("coerce", &arguments[0])?)),
        "VECTOR" | "SIMPLE-VECTOR" => {
            Ok(Value::vector(sequence_elements("coerce", &arguments[0])?))
        }
        "STRING" | "SIMPLE-STRING" => {
            let result = match &arguments[0] {
                Value::Nil
                | Value::Boolean(_)
                | Value::String(_)
                | Value::Symbol(_)
                | Value::UninternedSymbol(_)
                | Value::Keyword(_)
                | Value::SymbolExact(_)
                | Value::KeywordExact(_)
                | Value::Character(_) => string_designator("coerce", &arguments[0])?,
                value => sequence_elements("coerce", value)?
                    .into_iter()
                    .map(|item| character_argument("coerce", &item))
                    .collect::<Result<String, RuntimeError>>()?,
            };
            Ok(Value::string(result))
        }
        "SEQUENCE" => match &arguments[0] {
            Value::Nil | Value::List(_) | Value::Vector(_) | Value::String(_) => {
                Ok(arguments[0].clone())
            }
            value => Err(type_error("coerce", "a sequence", value)),
        },
        "CHARACTER" => match &arguments[0] {
            Value::Character(_) => Ok(arguments[0].clone()),
            value => Err(type_error("coerce", "a character", value)),
        },
        _ => Err(RuntimeError::InvalidForm {
            message: format!("coerce does not support result type {result_type}"),
            span: None,
        }),
    }
}

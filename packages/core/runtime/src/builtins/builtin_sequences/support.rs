use super::{array_option_name, integer_argument, type_error};
use crate::{RuntimeError, Value};

pub fn sequence_bounds(
    function: &str,
    length: usize,
    options: &[Value],
) -> Result<(usize, usize), RuntimeError> {
    let mut start = 0;
    let mut end = length;
    for pair in options.as_chunks::<2>().0 {
        match array_option_name(function, &pair[0])?.as_str() {
            "START" => start = index_argument(function, &pair[1])?,
            "END" => end = index_argument(function, &pair[1])?,
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bounds are invalid"),
            span: None,
        });
    }
    Ok((start, end))
}

pub fn replace_bounds(
    first_length: usize,
    second_length: usize,
    options: &[Value],
) -> Result<(usize, usize, usize, usize), RuntimeError> {
    let mut start1 = 0;
    let mut end1 = first_length;
    let mut start2 = 0;
    let mut end2 = second_length;
    for pair in options.as_chunks::<2>().0 {
        let option = array_option_name("replace", &pair[0])?;
        match option.as_str() {
            "START1" => start1 = index_argument("replace", &pair[1])?,
            "END1" => end1 = index_argument("replace", &pair[1])?,
            "START2" => start2 = index_argument("replace", &pair[1])?,
            "END2" => end2 = index_argument("replace", &pair[1])?,
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("replace does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start1 > end1 || end1 > first_length || start2 > end2 || end2 > second_length {
        return Err(RuntimeError::InvalidForm {
            message: "replace bounds are invalid".to_string(),
            span: None,
        });
    }
    Ok((start1, end1, start2, end2))
}

pub fn sequence_elements(function: &str, value: &Value) -> Result<Vec<Value>, RuntimeError> {
    match value {
        Value::Nil => Ok(Vec::new()),
        Value::List(items) | Value::Vector(items) => Ok(items.as_ref().clone()),
        Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
        _ => Err(type_error(function, "sequence", value)),
    }
}

pub fn rebuild_sequence(
    function: &str,
    template: &Value,
    items: Vec<Value>,
) -> Result<Value, RuntimeError> {
    match template {
        Value::Nil | Value::List(_) => Ok(Value::list(items)),
        Value::Vector(_) => Ok(Value::vector(items)),
        Value::String(_) => {
            let mut result = String::new();
            for item in items {
                let Value::Character(character) = item else {
                    return Err(type_error(
                        function,
                        "characters for a string sequence",
                        &item,
                    ));
                };
                result.push(character);
            }
            Ok(Value::string(result))
        }
        value => Err(type_error(function, "sequence", value)),
    }
}

pub fn sequence_length(value: &Value) -> Option<usize> {
    match value {
        Value::Nil => Some(0),
        Value::List(items) | Value::Vector(items) => Some(items.len()),
        Value::String(value) => Some(value.chars().count()),
        _ => None,
    }
}

pub fn index_argument(function: &str, value: &Value) -> Result<usize, RuntimeError> {
    let index = integer_argument(function, value)?;
    usize::try_from(index).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} index must be non-negative"),
        span: None,
    })
}

pub fn integer_from_usize(function: &str, value: usize) -> Result<Value, RuntimeError> {
    i64::try_from(value)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::InvalidForm {
            message: format!("{function} result is too large for an integer"),
            span: None,
        })
}

pub fn out_of_bounds(function: &str, index: usize) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} index {index} is out of bounds"),
        span: None,
    }
}

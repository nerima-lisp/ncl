use super::*;

pub(crate) fn length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) | Value::Vector(items) => items.len(),
        Value::String(value) => value.chars().count(),
        _ => {
            return Err(type_error("length", "sequence", &arguments[0]));
        }
    };
    Ok(Value::Integer(length as i64))
}

pub(crate) fn nth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nth", 2)?;
    let Some(items) = arguments[1].list_items() else {
        return Err(type_error("nth", "list", &arguments[1]));
    };
    let index = index_argument("nth", &arguments[0])?;
    Ok(items.get(index).cloned().unwrap_or(Value::Nil))
}

pub(crate) fn elt(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "elt", 2)?;
    let index = index_argument("elt", &arguments[1])?;
    match &arguments[0] {
        Value::Nil => Err(out_of_bounds("elt", index)),
        Value::List(items) | Value::Vector(items) => items
            .get(index)
            .cloned()
            .ok_or_else(|| out_of_bounds("elt", index)),
        Value::String(value) => value
            .chars()
            .nth(index)
            .map(Value::Character)
            .ok_or_else(|| out_of_bounds("elt", index)),
        value => Err(type_error("elt", "sequence", value)),
    }
}

pub(crate) fn sequence_length(value: &Value) -> Option<usize> {
    match value {
        Value::Nil => Some(0),
        Value::List(items) | Value::Vector(items) => Some(items.len()),
        Value::String(value) => Some(value.chars().count()),
        _ => None,
    }
}

pub(crate) fn index_argument(function: &str, value: &Value) -> Result<usize, RuntimeError> {
    let index = integer_argument(function, value)?;
    usize::try_from(index).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} index must be non-negative"),
        span: None,
    })
}

pub(crate) fn out_of_bounds(function: &str, index: usize) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} index {index} is out of bounds"),
        span: None,
    }
}

pub(crate) fn endp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "endp", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::boolean(true)),
        Value::List(_) => Ok(Value::boolean(false)),
        value => Err(type_error("endp", "list", value)),
    }
}

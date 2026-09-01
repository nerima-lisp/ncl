use super::{exact, index_argument, integer_from_usize, out_of_bounds, type_error};
use crate::{RuntimeError, Value};

pub fn length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) | Value::Vector(items) => items.len(),
        Value::String(value) => value.chars().count(),
        _ => {
            return Err(type_error("length", "sequence", &arguments[0]));
        }
    };
    integer_from_usize("length", length)
}

pub fn nth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nth", 2)?;
    let Some(items) = arguments[1].list_items() else {
        return Err(type_error("nth", "list", &arguments[1]));
    };
    let index = index_argument("nth", &arguments[0])?;
    Ok(items.get(index).cloned().unwrap_or(Value::Nil))
}

pub fn second(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "second", 1)?;
    nth(&[Value::Integer(1), arguments[0].clone()])
}

pub fn third(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "third", 1)?;
    nth(&[Value::Integer(2), arguments[0].clone()])
}

pub fn fourth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "fourth", 1)?;
    nth(&[Value::Integer(3), arguments[0].clone()])
}

pub fn fifth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "fifth", 1)?;
    nth(&[Value::Integer(4), arguments[0].clone()])
}

pub fn sixth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sixth", 1)?;
    nth(&[Value::Integer(5), arguments[0].clone()])
}

pub fn seventh(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "seventh", 1)?;
    nth(&[Value::Integer(6), arguments[0].clone()])
}

pub fn eighth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eighth", 1)?;
    nth(&[Value::Integer(7), arguments[0].clone()])
}

pub fn ninth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ninth", 1)?;
    nth(&[Value::Integer(8), arguments[0].clone()])
}

pub fn tenth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tenth", 1)?;
    nth(&[Value::Integer(9), arguments[0].clone()])
}

pub fn elt(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nth_reports_a_type_error_for_a_non_list_second_argument() {
        assert!(matches!(
            nth(&[Value::Integer(0), Value::Integer(5)]),
            Err(RuntimeError::Type { .. })
        ));
    }
}

use super::{exact, index_argument, integer_from_usize, out_of_bounds, type_error};
use crate::{RuntimeError, Value};

pub fn length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::Cons(_) => arguments[0]
            .list_items()
            .ok_or_else(|| type_error("length", "proper sequence", &arguments[0]))?
            .len(),
        Value::Vector(items) => items.sequence_len(),
        Value::String(value) => value.chars().count(),
        _ => {
            return Err(type_error("length", "sequence", &arguments[0]));
        }
    };
    integer_from_usize("length", length)
}

pub fn nth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nth", 2)?;
    let index = index_argument("nth", &arguments[0])?;
    match arguments[1].nth_tail(index) {
        Some(Value::Cons(cell)) => Ok(cell.car()),
        Some(Value::Nil | Value::Boolean(false)) => Ok(Value::Nil),
        _ => Err(type_error("nth", "list", &arguments[1])),
    }
}

pub fn elt(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "elt", 2)?;
    let index = index_argument("elt", &arguments[1])?;
    match &arguments[0] {
        Value::Nil => Err(out_of_bounds("elt", index)),
        Value::Cons(_) => match arguments[0].nth_tail(index) {
            Some(Value::Cons(cell)) => Ok(cell.car()),
            _ => Err(out_of_bounds("elt", index)),
        },
        Value::Vector(items) => items
            .visible_snapshot()
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

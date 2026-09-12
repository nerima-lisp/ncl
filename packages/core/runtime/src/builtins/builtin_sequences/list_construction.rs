use super::{arity, array_option_name, exact, index_argument, integer_from_usize, type_error};
use crate::{RuntimeError, Value};

#[expect(clippy::unnecessary_wraps)]
pub fn list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::list(arguments.to_vec()))
}

pub fn list_star(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let Some((tail, prefix)) = arguments.split_last() else {
        return Err(arity("list*", "at least one", 0));
    };
    Ok(Value::dotted_list(prefix.to_vec(), tail.clone()))
}

pub fn make_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-list", "at least one", 0));
    }
    if !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "make-list",
            "a size and keyword/value pairs",
            arguments.len(),
        ));
    }

    let size = index_argument("make-list", &arguments[0])?;
    let mut initial_element = Value::Nil;
    for pair in arguments[1..].as_chunks::<2>().0 {
        match array_option_name("make-list", &pair[0])?.as_str() {
            "INITIAL-ELEMENT" => initial_element = pair[1].clone(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-list does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    Ok(Value::list(vec![initial_element; size]))
}

pub fn values_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "values-list", 1)?;
    let Some(values) = arguments[0].list_items() else {
        return Err(type_error("values-list", "list", &arguments[0]));
    };
    Ok(Value::values(values))
}

pub fn list_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "list-length", 1)?;
    match arguments[0].list_parts() {
        Some((items, Value::Nil | Value::Boolean(false))) => {
            integer_from_usize("list-length", items.len())
        }
        None if matches!(arguments[0], Value::Cons(_)) => Ok(Value::Nil),
        _ => Err(type_error(
            "list-length",
            "proper or circular list",
            &arguments[0],
        )),
    }
}

pub fn acons(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acons", 3)?;
    if !matches!(
        arguments[2],
        Value::Nil | Value::Boolean(false) | Value::Cons(_)
    ) {
        return Err(type_error("acons", "list", &arguments[2]));
    }
    Ok(Value::cons(
        Value::cons(arguments[0].clone(), arguments[1].clone()),
        arguments[2].clone(),
    ))
}

pub fn pairlis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("pairlis", "2 or 3", arguments.len()));
    }
    let Some(keys) = arguments[0].list_items() else {
        return Err(type_error("pairlis", "list", &arguments[0]));
    };
    let Some(values) = arguments[1].list_items() else {
        return Err(type_error("pairlis", "list", &arguments[1]));
    };
    if keys.len() != values.len() {
        return Err(RuntimeError::InvalidForm {
            message: "pairlis requires lists of equal length".to_string(),
            span: None,
        });
    }
    let mut result = arguments.get(2).cloned().unwrap_or(Value::Nil);
    if !matches!(result, Value::Nil | Value::Boolean(false) | Value::Cons(_)) {
        return Err(type_error("pairlis", "list", &result));
    }
    for (key, value) in keys.into_iter().zip(values) {
        result = Value::cons(Value::cons(key, value), result);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acons_rejects_a_non_list_alist_argument() {
        assert!(matches!(
            acons(&[Value::Integer(1), Value::Integer(2), Value::Integer(3)]),
            Err(RuntimeError::Type { .. })
        ));
    }

    #[test]
    fn pairlis_rejects_non_list_keys_and_values() {
        let list = Value::list(vec![Value::Integer(1)]);
        assert!(matches!(
            pairlis(&[Value::Integer(1), list.clone()]),
            Err(RuntimeError::Type { .. })
        ));
        assert!(matches!(
            pairlis(&[list, Value::Integer(1)]),
            Err(RuntimeError::Type { .. })
        ));
    }
}

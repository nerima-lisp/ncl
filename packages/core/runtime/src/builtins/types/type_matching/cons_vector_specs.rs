use crate::builtins::types::type_matching::dispatch::type_matches_designator;
use crate::builtins::types::type_matching::spec_utils::{
    require_type_spec_arity, type_matches_element_spec, type_spec_size,
};
use crate::{RuntimeError, Value};

pub(in crate::builtins::types::type_matching) fn cons_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "CONS", arguments, 0, 2)?;
    let Some((car, cdr)) = cons_parts(value) else {
        return Ok(false);
    };
    if let Some(car_type) = arguments.first()
        && !type_matches_designator(function, &car, car_type)?
    {
        return Ok(false);
    }
    if let Some(cdr_type) = arguments.get(1)
        && !type_matches_designator(function, &cdr, cdr_type)?
    {
        return Ok(false);
    }
    Ok(true)
}

fn cons_parts(value: &Value) -> Option<(Value, Value)> {
    match value {
        Value::List(items) if !items.is_empty() => {
            let items = items.as_ref();
            let tail = if items.len() == 1 {
                Value::Nil
            } else {
                Value::list(items[1..].to_vec())
            };
            Some((items[0].clone(), tail))
        }
        Value::DottedList { items, tail } if !items.is_empty() => {
            Some((items[0].clone(), (*tail).as_ref().clone()))
        }
        _ => None,
    }
}

pub(in crate::builtins::types::type_matching) fn vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "VECTOR", arguments, 0, 2)?;
    let expected_size = arguments
        .get(1)
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    if expected_size.is_some_and(|size| size != items.len()) {
        return Ok(false);
    }
    if let Some(element_type) = arguments.first() {
        for item in &items {
            if !type_matches_element_spec(function, item, element_type)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(in crate::builtins::types::type_matching) fn simple_vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "SIMPLE-VECTOR", arguments, 0, 1)?;
    let expected_size = arguments
        .first()
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    Ok(expected_size.is_none_or(|size| size == items.len()))
}

pub(in crate::builtins::types::type_matching) fn bit_vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "BIT-VECTOR", arguments, 0, 1)?;
    let expected_size = arguments
        .first()
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    if expected_size.is_some_and(|size| size != items.len()) {
        return Ok(false);
    }
    Ok(items.iter().all(is_bit_value))
}

pub(in crate::builtins::types::type_matching) fn is_bit_vector_value(value: &Value) -> bool {
    matches!(value, Value::Vector(items) if items.borrow().iter().all(is_bit_value))
}

pub(in crate::builtins::types::type_matching) fn is_simple_bit_vector_value(value: &Value) -> bool {
    is_bit_vector_value(value)
        && value.vector_adjustable() != Some(true)
        && !value.array_has_fill_pointer().unwrap_or(false)
        && !value.is_displaced()
}

pub(in crate::builtins::types::type_matching) const fn is_bit_value(value: &Value) -> bool {
    matches!(value, Value::Integer(bit) if *bit == 0 || *bit == 1)
}

use crate::builtins::builtin_array_helpers::{array_elements, dimensions_for_array};
use crate::builtins::builtin_helpers::type_error;
use crate::builtins::types::type_matching::spec_utils::{
    invalid_type_spec, is_type_wildcard, require_type_spec_arity, type_matches_element_spec,
};
use crate::{RuntimeError, Value};

pub(in crate::builtins::types::type_matching) fn array_type_matches(
    function: &str,
    operator: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, operator, arguments, 0, 2)?;
    let Some(actual_dimensions) = dimensions_for_array(value) else {
        return Ok(false);
    };
    if operator == "SIMPLE-ARRAY" && !is_simple_array_value(value) {
        return Ok(false);
    }
    if let Some(expected_dimensions) = arguments.get(1)
        && !array_dimensions_match(function, expected_dimensions, &actual_dimensions)?
    {
        return Ok(false);
    }
    let Some(elements) = array_elements(value) else {
        return Ok(false);
    };
    if let Some(element_type) = arguments.first() {
        for element in &elements {
            if !type_matches_element_spec(function, element, element_type)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(in crate::builtins::types::type_matching) fn is_simple_array_value(value: &Value) -> bool {
    dimensions_for_array(value).is_some()
        && !value.array_adjustable().unwrap_or(false)
        && !value.array_has_fill_pointer().unwrap_or(false)
        && !value.is_displaced()
}

fn array_dimensions_match(
    function: &str,
    expected: &Value,
    actual: &[usize],
) -> Result<bool, RuntimeError> {
    if is_type_wildcard(expected) {
        return Ok(true);
    }
    match expected {
        Value::Nil | Value::Boolean(false) => Ok(actual.is_empty()),
        Value::Integer(rank) => {
            let rank = usize::try_from(*rank)
                .map_err(|_| invalid_type_spec(function, "array rank must be non-negative"))?;
            Ok(actual.len() == rank)
        }
        Value::List(dimensions) => {
            let dimensions = dimensions.as_ref();
            if dimensions.len() != actual.len() {
                return Ok(false);
            }
            for (dimension, actual) in dimensions.iter().zip(actual) {
                if is_type_wildcard(dimension) {
                    continue;
                }
                let Value::Integer(expected) = dimension else {
                    return Err(type_error(function, "array dimension or *", dimension));
                };
                let expected = usize::try_from(*expected).map_err(|_| {
                    invalid_type_spec(function, "array dimensions must be non-negative")
                })?;
                if expected != *actual {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        value => Err(type_error(function, "array dimensions", value)),
    }
}

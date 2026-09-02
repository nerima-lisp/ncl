use crate::environment::normalize_name;

use super::{index_argument, out_of_bounds, type_error};
use crate::{RuntimeError, Value};

pub(super) fn parse_array_dimensions(
    function: &str,
    value: &Value,
) -> Result<Vec<usize>, RuntimeError> {
    match value {
        Value::Integer(_) => Ok(vec![index_argument(function, value)?]),
        Value::Nil => Ok(Vec::new()),
        Value::List(_) | Value::Vector(_) => {
            let Some(items) = sequence_items(value) else {
                return Err(type_error(
                    function,
                    "integer or sequence of integers",
                    value,
                ));
            };
            items
                .iter()
                .map(|item| index_argument(function, item))
                .collect()
        }
        other => Err(type_error(
            function,
            "integer or sequence of integers",
            other,
        )),
    }
}

pub(super) fn array_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
        other => Err(type_error(function, "keyword", other)),
    }
}

pub(super) fn flatten_array_contents(
    function: &str,
    contents: &Value,
    dimensions: &[usize],
    output: &mut Vec<Value>,
) -> Result<(), RuntimeError> {
    if dimensions.is_empty() {
        output.push(contents.clone());
        return Ok(());
    }
    let Some(items) = sequence_items(contents) else {
        return Err(type_error(
            function,
            "nested sequence for :initial-contents",
            contents,
        ));
    };
    if items.len() != dimensions[0] {
        return Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} :initial-contents expected {} elements, got {}",
                dimensions[0],
                items.len()
            ),
            span: None,
        });
    }
    if dimensions.len() == 1 {
        output.extend(items);
    } else {
        for item in items {
            flatten_array_contents(function, &item, &dimensions[1..], output)?;
        }
    }
    Ok(())
}

pub(super) fn array_coordinate_index(
    function: &str,
    dimensions: &[usize],
    indices: &[Value],
) -> Result<usize, RuntimeError> {
    let mut offset = 0_usize;
    for (axis, (dimension, value)) in dimensions.iter().zip(indices).enumerate() {
        let index = index_argument(function, value)?;
        if index >= *dimension {
            return Err(out_of_bounds(function, index));
        }
        let stride = dimensions[axis + 1..]
            .iter()
            .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
        let contribution = index
            .checked_mul(stride)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
        offset = offset
            .checked_add(contribution)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
    }
    Ok(offset)
}

pub(super) fn array_total_size_for(
    function: &str,
    dimensions: &[usize],
) -> Result<usize, RuntimeError> {
    dimensions.iter().try_fold(1_usize, |total, dimension| {
        total
            .checked_mul(*dimension)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} array is too large"),
                span: None,
            })
    })
}

pub(super) fn dimensions_for_array(value: &Value) -> Option<Vec<usize>> {
    match value {
        Value::Vector(items) => Some(vec![items.borrow().len()]),
        Value::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
        Value::String(value) => Some(vec![value.chars().count()]),
        Value::MutableString(value) => Some(vec![value.borrow().chars().count()]),
        _ => None,
    }
}

pub(super) fn array_elements(value: &Value) -> Option<Vec<Value>> {
    value.vector_items().or_else(|| value.array_items()).or_else(|| {
        value
            .string_contents()
            .map(|text| text.chars().map(Value::Character).collect())
    })
}

pub(super) fn sequence_items(value: &Value) -> Option<Vec<Value>> {
    value.list_items().or_else(|| value.vector_items())
}

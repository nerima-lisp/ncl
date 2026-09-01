use crate::builtins::{
    arity, array_coordinate_index, array_elements, array_total_size_for, dimensions_for_array,
    exact, index_argument, integer_from_usize, out_of_bounds, type_error,
};
use crate::{RuntimeError, Value};

pub fn aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("aref", "at least one", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("aref", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "aref",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    let index = array_coordinate_index("aref", &dimensions, &arguments[1..])?;
    array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("aref", index))
}

pub fn svref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "svref", 2)?;
    let index = index_argument("svref", &arguments[1])?;
    let Value::Vector(items) = &arguments[0] else {
        return Err(type_error("svref", "simple-vector", &arguments[0]));
    };
    items
        .borrow()
        .get(index)
        .cloned()
        .ok_or_else(|| out_of_bounds("svref", index))
}

fn bit_value(function: &str, value: &Value) -> Result<(), RuntimeError> {
    match value {
        Value::Integer(bit) if *bit == 0 || *bit == 1 => Ok(()),
        _ => Err(type_error(function, "bit", value)),
    }
}

pub fn bit(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("bit", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("bit", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "bit",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    let index = array_coordinate_index("bit", &dimensions, &arguments[1..])?;
    let value = array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("bit", index))?;
    bit_value("bit", &value)?;
    Ok(value)
}

pub fn row_major_aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "row-major-aref", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("row-major-aref", "array", &arguments[0]))?;
    let index = index_argument("row-major-aref", &arguments[1])?;
    let total_size = array_total_size_for("row-major-aref", &dimensions)?;
    if index >= total_size {
        return Err(out_of_bounds("row-major-aref", index));
    }
    array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("row-major-aref", index))
}

pub fn array_row_major_index(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("array-row-major-index", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-row-major-index", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "array-row-major-index",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    integer_from_usize(
        "array-row-major-index",
        array_coordinate_index("array-row-major-index", &dimensions, &arguments[1..])?,
    )
}

pub fn array_in_bounds_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("array-in-bounds-p", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-in-bounds-p", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "array-in-bounds-p",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    for (dimension, value) in dimensions.iter().zip(&arguments[1..]) {
        if index_argument("array-in-bounds-p", value)? >= *dimension {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::boolean(true))
}

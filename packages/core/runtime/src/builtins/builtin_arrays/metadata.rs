use crate::builtins::{
    array_total_size_for, dimensions_for_array, exact, index_argument, integer_from_usize,
    out_of_bounds, type_error,
};
use crate::{RuntimeError, Value};

pub fn array_element_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-element-type", 1)?;
    let elements = arguments[0]
        .array_storage()
        .ok_or_else(|| type_error("array-element-type", "array", &arguments[0]))?;
    Ok(elements.element_type())
}

pub fn simple_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-array-p", 1)?;
    let simple = dimensions_for_array(&arguments[0]).is_some_and(|_| {
        arguments[0].array_storage().is_some_and(|elements| {
            !elements.has_fill_pointer() && !elements.is_adjustable() && !elements.is_displaced()
        })
    });
    Ok(Value::boolean(simple))
}

pub fn adjustable_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "adjustable-array-p", 1)?;
    let elements = arguments[0]
        .array_storage()
        .ok_or_else(|| type_error("adjustable-array-p", "array", &arguments[0]))?;
    Ok(Value::boolean(elements.is_adjustable()))
}

pub fn array_displacement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-displacement", 1)?;
    let elements = arguments[0]
        .array_storage()
        .ok_or_else(|| type_error("array-displacement", "array", &arguments[0]))?;
    let (source, offset) = elements.displacement().unwrap_or((Value::Nil, 0));
    Ok(Value::values(vec![
        source,
        integer_from_usize("array-displacement", offset)?,
    ]))
}

pub fn arrayp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "arrayp", 1)?;
    Ok(Value::boolean(
        dimensions_for_array(&arguments[0]).is_some(),
    ))
}

pub fn array_rank(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-rank", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-rank", "array", &arguments[0]))?;
    integer_from_usize("array-rank", dimensions.len())
}

pub fn array_dimensions(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimensions", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimensions", "array", &arguments[0]))?;
    dimensions
        .into_iter()
        .map(|dimension| integer_from_usize("array-dimensions", dimension))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::list)
}

pub fn array_dimension(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimension", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimension", "array", &arguments[0]))?;
    let index = index_argument("array-dimension", &arguments[1])?;
    dimensions
        .get(index)
        .copied()
        .map(|dimension| integer_from_usize("array-dimension", dimension))
        .transpose()?
        .ok_or_else(|| out_of_bounds("array-dimension", index))
}

pub fn array_total_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-total-size", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-total-size", "array", &arguments[0]))?;
    integer_from_usize(
        "array-total-size",
        array_total_size_for("array-total-size", &dimensions)?,
    )
}

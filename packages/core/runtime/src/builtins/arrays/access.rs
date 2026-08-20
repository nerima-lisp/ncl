fn aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn svref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "svref", 2)?;
    let index = index_argument("svref", &arguments[1])?;
    let items = arguments[0]
        .vector_items()
        .ok_or_else(|| type_error("svref", "simple-vector", &arguments[0]))?;
    items
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

fn bit(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn sbit(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("sbit", "array and subscripts", 0));
    }
    if !simple_bit_array_value(&arguments[0]) {
        return Err(type_error("sbit", "simple bit array", &arguments[0]));
    }
    bit(arguments)
}

fn row_major_aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn array_row_major_index(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
    Ok(Value::Integer(
        array_coordinate_index("array-row-major-index", &dimensions, &arguments[1..])? as i64,
    ))
}

fn array_in_bounds_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

fn array_element_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-element-type", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-element-type", "array", &arguments[0]))?;
    Ok(arguments[0]
        .array_element_type_value()
        .expect("array values carry element type"))
}

fn array_has_fill_pointer_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-has-fill-pointer-p", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-has-fill-pointer-p", "array", &arguments[0]))?;
    Ok(Value::boolean(array_has_fill_pointer_value(&arguments[0])))
}

fn adjustable_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "adjustable-array-p", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("adjustable-array-p", "array", &arguments[0]))?;
    Ok(Value::boolean(arguments[0].is_adjustable_array()))
}

fn array_displacement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-displacement", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-displacement", "array", &arguments[0]))?;
    if let Some((displaced_to, displaced_index_offset)) = arguments[0].array_displacement_value() {
        Ok(Value::values(vec![
            displaced_to,
            Value::Integer(displaced_index_offset as i64),
        ]))
    } else {
        Ok(Value::values(vec![Value::Nil, Value::Integer(0)]))
    }
}

fn simple_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-array-p", 1)?;
    Ok(Value::boolean(simple_array_value(&arguments[0])))
}

fn arrayp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "arrayp", 1)?;
    Ok(Value::boolean(
        dimensions_for_array(&arguments[0]).is_some(),
    ))
}

fn array_rank(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-rank", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-rank", "array", &arguments[0]))?;
    Ok(Value::Integer(dimensions.len() as i64))
}

fn array_dimensions(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimensions", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimensions", "array", &arguments[0]))?;
    Ok(Value::list(
        dimensions
            .into_iter()
            .map(|dimension| Value::Integer(dimension as i64))
            .collect(),
    ))
}

fn array_dimension(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimension", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimension", "array", &arguments[0]))?;
    let index = index_argument("array-dimension", &arguments[1])?;
    dimensions
        .get(index)
        .copied()
        .map(|dimension| Value::Integer(dimension as i64))
        .ok_or_else(|| out_of_bounds("array-dimension", index))
}

fn array_total_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-total-size", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-total-size", "array", &arguments[0]))?;
    Ok(Value::Integer(
        array_total_size_for("array-total-size", &dimensions)? as i64,
    ))
}

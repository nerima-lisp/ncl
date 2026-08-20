fn parse_array_dimensions(function: &str, value: &Value) -> Result<Vec<usize>, RuntimeError> {
    match value {
        Value::Integer(_) => Ok(vec![index_argument(function, value)?]),
        Value::Nil => Ok(Vec::new()),
        _ if sequence_items(value).is_some() => {
            let items = sequence_items(value).expect("sequence has sequence items");
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

fn array_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
        other => Err(type_error(function, "keyword", other)),
    }
}

fn flatten_array_contents(
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

fn array_coordinate_index(
    function: &str,
    dimensions: &[usize],
    indices: &[Value],
) -> Result<usize, RuntimeError> {
    let mut offset: usize = 0;
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

fn array_total_size_for(function: &str, dimensions: &[usize]) -> Result<usize, RuntimeError> {
    dimensions.iter().try_fold(1_usize, |total, dimension| {
        total
            .checked_mul(*dimension)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} array is too large"),
                span: None,
            })
    })
}

fn dimensions_for_array(value: &Value) -> Option<Vec<usize>> {
    if let Some(items) = value.vector_items() {
        return Some(vec![items.len()]);
    }
    match value {
        Value::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
        _ => None,
    }
}

fn array_elements(value: &Value) -> Option<Vec<Value>> {
    value.vector_items().or_else(|| value.array_items())
}

fn array_has_fill_pointer_value(value: &Value) -> bool {
    value.vector_fill_pointer().is_some()
}

fn simple_array_value(value: &Value) -> bool {
    match value {
        Value::Vector { .. } => {
            !array_has_fill_pointer_value(value)
                && !value.is_adjustable_array()
                && value.array_displacement_value().is_none()
        }
        Value::Array { .. } => {
            !value.is_adjustable_array() && value.array_displacement_value().is_none()
        }
        _ => false,
    }
}

fn simple_bit_array_value(value: &Value) -> bool {
    simple_array_value(value)
        && matches!(
            value.array_element_type_value(),
            Some(Value::Symbol(type_name)) if type_name.as_ref() == "BIT"
        )
}

fn displaced_array_arguments(
    function: &str,
    dimensions: &[usize],
    displaced_to: Option<Value>,
    displaced_index_offset: Option<Value>,
) -> Result<Option<DisplacedArray>, RuntimeError> {
    match displaced_to {
        Some(displaced_to) => {
            dimensions_for_array(&displaced_to)
                .ok_or_else(|| type_error(function, "array", &displaced_to))?;
            let displaced_index_offset = match displaced_index_offset {
                Some(value) => index_argument(function, &value)?,
                None => 0,
            };
            let total_size = array_total_size_for(function, dimensions)?;
            let effective_offset = displaced_to
                .array_displacement_value()
                .map(|(_, offset)| offset)
                .unwrap_or(0)
                .checked_add(displaced_index_offset)
                .ok_or_else(|| RuntimeError::InvalidForm {
                    message: format!("{function} displacement is too large"),
                    span: None,
                })?;
            let source_storage = displaced_to
                .array_storage()
                .expect("array values carry shared storage");
            let source_size = source_storage.borrow().len();
            let end = effective_offset.checked_add(total_size).ok_or_else(|| {
                RuntimeError::InvalidForm {
                    message: format!("{function} displacement is too large"),
                    span: None,
                }
            })?;
            if end > source_size {
                return Err(RuntimeError::InvalidForm {
                    message: format!(
                        "{function} displacement range {}..{} is out of bounds for source size {}",
                        effective_offset, end, source_size
                    ),
                    span: None,
                });
            }
            Ok(Some(DisplacedArray {
                displaced_to: Some(displaced_to),
                displaced_index_offset: effective_offset,
                storage: source_storage,
            }))
        }
        None => {
            if displaced_index_offset.is_some() {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} :displaced-index-offset requires :displaced-to"),
                    span: None,
                });
            }
            Ok(None)
        }
    }
}

fn array_fill_pointer(function: &str, value: &Value, length: usize) -> Result<usize, RuntimeError> {
    if value
        .symbol_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("T"))
    {
        return Ok(length);
    }
    let fill_pointer = index_argument(function, value)?;
    if fill_pointer > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} :fill-pointer {fill_pointer} is out of bounds"),
            span: None,
        });
    }
    Ok(fill_pointer)
}

fn sequence_items(value: &Value) -> Option<Vec<Value>> {
    value.list_items().or_else(|| value.vector_items())
}

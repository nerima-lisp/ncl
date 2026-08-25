use super::*;
use crate::value::ArrayElementType;

pub(super) fn vector(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::vector(arguments.to_vec()))
}

pub(super) fn fill_pointer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "fill-pointer", 1)?;
    let Some(pointer) = arguments[0].array_fill_pointer() else {
        return Err(type_error(
            "fill-pointer",
            "an array with a fill pointer",
            &arguments[0],
        ));
    };
    Ok(Value::Integer(pointer as i64))
}

pub(super) fn vector_push(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vector-push", 2)?;
    let Value::Array {
        dimensions,
        elements,
        element_type: _,
        fill_pointer: Some(pointer),
        ..
    } = &arguments[1]
    else {
        return Err(type_error(
            "vector-push",
            "a vector with a fill pointer",
            &arguments[1],
        ));
    };
    if dimensions.len() != 1 {
        return Err(type_error(
            "vector-push",
            "a one-dimensional vector with a fill pointer",
            &arguments[1],
        ));
    }
    if !arguments[1].accepts_array_element(&arguments[0]) {
        return Err(type_error("vector-push", "an element of the vector type", &arguments[0]));
    }
    let index = *pointer.borrow();
    let mut elements = elements.borrow_mut();
    let Some(slot) = elements.get_mut(index) else {
        return Ok(Value::Nil);
    };
    *slot = arguments[0].clone();
    *pointer.borrow_mut() = index + 1;
    Ok(Value::Integer(index as i64))
}

pub(super) fn vector_push_extend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("vector-push-extend", "two or three", arguments.len()));
    }
    let Value::Array {
        dimensions,
        elements,
        element_type,
        fill_pointer: Some(pointer),
        adjustable,
        ..
    } = &arguments[1]
    else {
        return Err(type_error(
            "vector-push-extend",
            "a vector with a fill pointer",
            &arguments[1],
        ));
    };
    if dimensions.len() != 1 {
        return Err(type_error(
            "vector-push-extend",
            "a one-dimensional vector with a fill pointer",
            &arguments[1],
        ));
    }
    if !*adjustable {
        return Err(type_error(
            "vector-push-extend",
            "an adjustable vector with a fill pointer",
            &arguments[1],
        ));
    }
    if !arguments[1].accepts_array_element(&arguments[0]) {
        return Err(type_error(
            "vector-push-extend",
            "an element of the vector type",
            &arguments[0],
        ));
    }
    let extension = match arguments.get(2) {
        None => 1,
        Some(value) => {
            let extension = index_argument("vector-push-extend", value)?;
            if extension == 0 {
                return Err(RuntimeError::InvalidForm {
                    message: "vector-push-extend extension must be positive".to_owned(),
                    span: None,
                });
            }
            extension
        }
    };
    let index = *pointer.borrow();
    {
        let mut elements = elements.borrow_mut();
        if index == elements.len() {
            let new_length = elements.len().checked_add(extension).ok_or_else(|| {
                RuntimeError::InvalidForm {
                    message: "vector-push-extend size is too large".to_owned(),
                    span: None,
                }
            })?;
            let default = match *element_type {
                ArrayElementType::T => Value::Nil,
                ArrayElementType::Character => Value::Character('\0'),
                ArrayElementType::Bit => Value::Integer(0),
            };
            elements.resize(new_length, default);
        }
        elements[index] = arguments[0].clone();
    }
    *pointer.borrow_mut() = index + 1;
    Ok(Value::Integer(index as i64))
}

pub(super) fn vector_pop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vector-pop", 1)?;
    let Value::Array {
        dimensions,
        elements,
        fill_pointer: Some(pointer),
        ..
    } = &arguments[0]
    else {
        return Err(type_error(
            "vector-pop",
            "a vector with a fill pointer",
            &arguments[0],
        ));
    };
    if dimensions.len() != 1 {
        return Err(type_error(
            "vector-pop",
            "a one-dimensional vector with a fill pointer",
            &arguments[0],
        ));
    }
    let index = *pointer.borrow();
    if index == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "vector-pop requires a non-empty vector".to_owned(),
            span: None,
        });
    }
    let index = index - 1;
    let value = elements
        .borrow()
        .get(index)
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: "vector-pop fill pointer is out of bounds".to_owned(),
            span: None,
        })?;
    *pointer.borrow_mut() = index;
    Ok(value)
}

pub(super) fn adjustable_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "adjustable-array-p", 1)?;
    Ok(Value::boolean(arguments[0].is_adjustable_array()))
}

pub(super) fn array_has_fill_pointer_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-has-fill-pointer-p", 1)?;
    Ok(Value::boolean(arguments[0].has_fill_pointer()))
}

pub(super) fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let mut element_type = None;
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut fill_pointer = None;
    let mut adjustable = false;
    if (arguments.len() - 1) % 2 != 0 {
        return Err(arity(
            "make-array",
            "one dimension and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[1..].chunks_exact(2) {
        let name = array_option_name("make-array", &pair[0])?;
        match name.as_str() {
            "ELEMENT-TYPE" => element_type = Some(pair[1].clone()),
            "INITIAL-ELEMENT" => {
                if initial_contents.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_element = Some(pair[1].clone());
            }
            "INITIAL-CONTENTS" => {
                if initial_element.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_contents = Some(pair[1].clone());
            }
            "FILL-POINTER" => {
                fill_pointer = if matches!(&pair[1], Value::Nil) {
                    None
                } else {
                    let pointer = integer_argument("make-array", &pair[1])?;
                    Some(usize::try_from(pointer).map_err(|_| RuntimeError::InvalidForm {
                        message: "make-array :fill-pointer must be non-negative".to_owned(),
                        span: None,
                    })?)
                };
            }
            "ADJUSTABLE" => adjustable = pair[1].is_truthy(),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let element_type = element_type
        .map(|element_type| parse_array_element_type("make-array", &element_type))
        .transpose()?
        .unwrap_or(ArrayElementType::T);
    let total_size = array_total_size_for("make-array", &dimensions)?;
    let elements = if let Some(contents) = initial_contents {
        let mut elements = Vec::with_capacity(total_size);
        flatten_array_contents("make-array", &contents, &dimensions, &mut elements)?;
        elements
    } else {
        let default = match element_type {
            ArrayElementType::T => Value::Nil,
            ArrayElementType::Character => Value::Character('\0'),
            ArrayElementType::Bit => Value::Integer(0),
        };
        vec![initial_element.unwrap_or(default); total_size]
    };
    validate_array_elements("make-array", &elements, element_type)?;
    if fill_pointer.is_some() && dimensions.len() != 1 {
        return Err(RuntimeError::InvalidForm {
            message: "make-array :fill-pointer requires a one-dimensional array".to_owned(),
            span: None,
        });
    }
    if let Some(fill_pointer) = fill_pointer.as_ref() {
        if *fill_pointer > total_size {
            return Err(RuntimeError::InvalidForm {
                message: "make-array :fill-pointer exceeds the array size".to_owned(),
                span: None,
            });
        }
    }
    if dimensions.len() == 1 {
        match element_type {
            ArrayElementType::T => Ok(Value::array_with_options(
                dimensions,
                elements,
                element_type,
                fill_pointer,
                adjustable,
            )),
            ArrayElementType::Character if fill_pointer.is_none() && !adjustable => {
                Ok(Value::string(
                    elements
                        .into_iter()
                        .map(|element| match element {
                            Value::Character(character) => character,
                            _ => unreachable!("validated character array element"),
                        })
                        .collect::<String>(),
                ))
            }
            ArrayElementType::Character | ArrayElementType::Bit => Ok(Value::array_with_options(
                dimensions,
                elements,
                element_type,
                fill_pointer,
                adjustable,
            )),
        }
    } else {
        Ok(Value::array_with_options(
            dimensions,
            elements,
            element_type,
            fill_pointer,
            adjustable,
        ))
    }
}

pub(super) fn adjust_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("adjust-array", "at least 2", arguments.len()));
    }
    let source = &arguments[0];
    let source_elements = array_elements(source)
        .ok_or_else(|| type_error("adjust-array", "array", source))?;
    let dimensions = parse_array_dimensions("adjust-array", &arguments[1])?;
    let mut element_type = source
        .array_element_type()
        .ok_or_else(|| type_error("adjust-array", "array", source))?;
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut fill_pointer = source.array_fill_pointer();
    let adjustable = source.is_adjustable_array();
    if (arguments.len() - 2) % 2 != 0 {
        return Err(arity(
            "adjust-array",
            "array, dimensions, and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[2..].chunks_exact(2) {
        let name = array_option_name("adjust-array", &pair[0])?;
        match name.as_str() {
            "ELEMENT-TYPE" => {
                element_type = parse_array_element_type("adjust-array", &pair[1])?;
            }
            "INITIAL-ELEMENT" => {
                if initial_contents.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message:
                            "adjust-array cannot combine :initial-element and :initial-contents"
                                .to_string(),
                        span: None,
                    });
                }
                initial_element = Some(pair[1].clone());
            }
            "INITIAL-CONTENTS" => {
                if initial_element.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message:
                            "adjust-array cannot combine :initial-element and :initial-contents"
                                .to_string(),
                        span: None,
                    });
                }
                initial_contents = Some(pair[1].clone());
            }
            "FILL-POINTER" => {
                fill_pointer = if matches!(&pair[1], Value::Nil) {
                    None
                } else {
                    let pointer = integer_argument("adjust-array", &pair[1])?;
                    Some(usize::try_from(pointer).map_err(|_| RuntimeError::InvalidForm {
                        message: "adjust-array :fill-pointer must be non-negative".to_owned(),
                        span: None,
                    })?)
                };
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("adjust-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("adjust-array", &dimensions)?;
    let elements = if let Some(contents) = initial_contents {
        let mut elements = Vec::with_capacity(total_size);
        flatten_array_contents("adjust-array", &contents, &dimensions, &mut elements)?;
        elements
    } else {
        let default = initial_element.unwrap_or_else(|| match element_type {
            ArrayElementType::T => Value::Nil,
            ArrayElementType::Character => Value::Character('\0'),
            ArrayElementType::Bit => Value::Integer(0),
        });
        let mut elements = vec![default; total_size];
        for (index, element) in source_elements.iter().enumerate().take(total_size) {
            elements[index] = element.clone();
        }
        elements
    };
    validate_array_elements("adjust-array", &elements, element_type)?;
    if fill_pointer.is_some() && dimensions.len() != 1 {
        return Err(RuntimeError::InvalidForm {
            message: "adjust-array :fill-pointer requires a one-dimensional array".to_owned(),
            span: None,
        });
    }
    if let Some(fill_pointer) = fill_pointer.as_ref() {
        if *fill_pointer > total_size {
            return Err(RuntimeError::InvalidForm {
                message: "adjust-array :fill-pointer exceeds the array size".to_owned(),
                span: None,
            });
        }
    }
    if dimensions.len() == 1
        && element_type == ArrayElementType::Character
        && fill_pointer.is_none()
        && !adjustable
    {
        return Ok(Value::string(
            elements
                .into_iter()
                .map(|element| match element {
                    Value::Character(character) => character,
                    _ => unreachable!("validated character array element"),
                })
                .collect::<String>(),
        ));
    }
    if dimensions.len() == 1
        && element_type == ArrayElementType::T
        && matches!(source, Value::Vector(_))
        && fill_pointer.is_none()
        && !adjustable
    {
        return Ok(Value::vector(elements));
    }
    Ok(Value::array_with_options(
        dimensions,
        elements,
        element_type,
        fill_pointer,
        adjustable,
    ))
}

fn parse_array_element_type(
    function: &str,
    value: &Value,
) -> Result<ArrayElementType, RuntimeError> {
    match type_designator_name(function, value)?.as_str() {
        "T" => Ok(ArrayElementType::T),
        "CHARACTER" => Ok(ArrayElementType::Character),
        "BIT" => Ok(ArrayElementType::Bit),
        element_type => Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} only supports :element-type T, CHARACTER, or BIT, got {element_type}"
            ),
            span: None,
        }),
    }
}

fn validate_array_elements(
    function: &str,
    elements: &[Value],
    element_type: ArrayElementType,
) -> Result<(), RuntimeError> {
    let expected = match element_type {
        ArrayElementType::T => return Ok(()),
        ArrayElementType::Character => "CHARACTER",
        ArrayElementType::Bit => "BIT",
    };
    let is_valid = |element: &&Value| match element_type {
        ArrayElementType::T => true,
        ArrayElementType::Character => matches!(element, Value::Character(_)),
        ArrayElementType::Bit => {
            matches!(element, Value::Integer(bit) if *bit == 0 || *bit == 1)
        }
    };
    if let Some(element) = elements.iter().find(|element| !is_valid(element)) {
        return Err(type_error(function, expected, element));
    }
    Ok(())
}

pub(super) fn aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn svref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "svref", 2)?;
    let index = index_argument("svref", &arguments[1])?;
    let Some(items) = (match &arguments[0] {
        Value::Vector(items) => Some(items.borrow().clone()),
        Value::Array {
            dimensions,
            elements,
            fill_pointer: None,
            adjustable: false,
            element_type: ArrayElementType::T,
        } if dimensions.len() == 1 => Some(elements.borrow().clone()),
        value if value.is_typed_vector() => value.vector_items(),
        _ => None,
    }) else {
        return Err(type_error("svref", "simple-vector", &arguments[0]));
    };
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

fn bit_array_parts(
    function: &str,
    value: &Value,
) -> Result<(Vec<usize>, Vec<Value>), RuntimeError> {
    match value {
        Value::Array {
            elements,
            element_type: ArrayElementType::Bit,
            ..
        } => Ok((
            value
                .array_dimensions()
                .expect("bit array dimensions are available"),
            elements.borrow().clone(),
        )),
        _ => Err(type_error(function, "bit-array", value)),
    }
}

fn simple_bit_vector_elements(function: &str, value: &Value) -> Result<Vec<Value>, RuntimeError> {
    let (dimensions, elements) = bit_array_parts(function, value)?;
    if dimensions.len() != 1 {
        return Err(type_error(function, "simple-bit-vector", value));
    }
    if value.has_fill_pointer() || value.is_adjustable_array() {
        return Err(type_error(function, "simple-bit-vector", value));
    }
    Ok(elements)
}

pub(super) fn bit(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("bit", "array and subscripts", 0));
    }
    let (dimensions, elements) = bit_array_parts("bit", &arguments[0])?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "bit",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    let index = array_coordinate_index("bit", &dimensions, &arguments[1..])?;
    let value = elements
        .get(index)
        .cloned()
        .ok_or_else(|| out_of_bounds("bit", index))?;
    bit_value("bit", &value)?;
    Ok(value)
}

pub(super) fn sbit(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sbit", 2)?;
    let elements = simple_bit_vector_elements("sbit", &arguments[0])?;
    let index = index_argument("sbit", &arguments[1])?;
    let value = elements
        .get(index)
        .cloned()
        .ok_or_else(|| out_of_bounds("sbit", index))?;
    bit_value("sbit", &value)?;
    Ok(value)
}

pub(super) fn bit_not(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("bit-not", "1 or 2", arguments.len()));
    }
    let (dimensions, source_elements) = bit_array_parts("bit-not", &arguments[0])?;
    let inverted_elements = source_elements
        .into_iter()
        .map(|value| match value {
            Value::Integer(0) => Ok(Value::Integer(1)),
            Value::Integer(1) => Ok(Value::Integer(0)),
            value => Err(type_error("bit-not", "bit", &value)),
        })
        .collect::<Result<Vec<_>, _>>()?;

    match arguments.get(1) {
        None | Some(Value::Nil) => Ok(Value::array_with_element_type(
            dimensions,
            inverted_elements,
            ArrayElementType::Bit,
        )),
        Some(Value::Boolean(true)) => {
            let Value::Array { elements, .. } = &arguments[0] else {
                unreachable!("bit-array validation succeeded before destination handling");
            };
            *elements.borrow_mut() = inverted_elements;
            Ok(arguments[0].clone())
        }
        Some(target) => {
            let Value::Array {
                elements: target_elements,
                element_type: ArrayElementType::Bit,
                ..
            } = target
            else {
                return Err(type_error("bit-not", "bit-array", target));
            };
            if target.array_dimensions().as_deref() != Some(dimensions.as_slice())
                || target_elements.borrow().len() != inverted_elements.len()
            {
                return Err(RuntimeError::InvalidForm {
                    message: "bit-not requires arrays with matching dimensions".to_owned(),
                    span: None,
                });
            }
            *target_elements.borrow_mut() = inverted_elements;
            Ok(target.clone())
        }
    }
}

pub(super) fn bit_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "bit-vector-p", 1)?;
    let is_bit_vector = matches!(
        &arguments[0],
        Value::Array {
            dimensions,
            elements,
            element_type: ArrayElementType::Bit,
            ..
        } if dimensions.len() == 1
            && elements
                .borrow()
                .iter()
                .all(|value| matches!(value, Value::Integer(bit) if *bit == 0 || *bit == 1))
    );
    Ok(Value::boolean(is_bit_vector))
}

pub(super) fn row_major_aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn array_row_major_index(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn array_in_bounds_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
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
        let index = integer_argument("array-in-bounds-p", value)?;
        let Ok(index) = usize::try_from(index) else {
            return Ok(Value::Nil);
        };
        if index >= *dimension {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::boolean(true))
}

pub(super) fn array_element_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-element-type", 1)?;
    let element_type = arguments[0]
        .array_element_type()
        .ok_or_else(|| type_error("array-element-type", "array", &arguments[0]))?;
    Ok(Value::symbol(element_type.name()))
}

pub(super) fn simple_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-array-p", 1)?;
    let simple = match &arguments[0] {
        Value::Array {
            fill_pointer,
            adjustable,
            ..
        } => fill_pointer.is_none() && !adjustable,
        value => dimensions_for_array(value).is_some(),
    };
    Ok(Value::boolean(simple))
}

pub(super) fn arrayp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "arrayp", 1)?;
    Ok(Value::boolean(
        dimensions_for_array(&arguments[0]).is_some(),
    ))
}

pub(super) fn array_rank(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-rank", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-rank", "array", &arguments[0]))?;
    Ok(Value::Integer(dimensions.len() as i64))
}

pub(super) fn array_dimensions(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn array_dimension(arguments: &[Value]) -> Result<Value, RuntimeError> {
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

pub(super) fn array_total_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-total-size", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-total-size", "array", &arguments[0]))?;
    Ok(Value::Integer(
        array_total_size_for("array-total-size", &dimensions)? as i64,
    ))
}

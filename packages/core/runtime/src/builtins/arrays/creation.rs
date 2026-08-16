fn vector(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::vector(arguments.to_vec()))
}

fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut fill_pointer = None;
    let mut element_type = None;
    let mut adjustable = false;
    let mut displaced_to = None;
    let mut displaced_index_offset = None;
    if !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "make-array",
            "one dimension and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[1..].chunks_exact(2) {
        let name = array_option_name("make-array", &pair[0])?;
        match name.as_str() {
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
            "FILL-POINTER" => fill_pointer = Some(pair[1].clone()),
            "ELEMENT-TYPE" => element_type = Some(pair[1].clone()),
            "ADJUSTABLE" => adjustable = !matches!(pair[1], Value::Nil),
            "DISPLACED-TO" => displaced_to = Some(pair[1].clone()),
            "DISPLACED-INDEX-OFFSET" => displaced_index_offset = Some(pair[1].clone()),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("make-array", &dimensions)?;
    if displaced_to.is_some() && (initial_element.is_some() || initial_contents.is_some()) {
        return Err(RuntimeError::InvalidForm {
            message:
                "make-array cannot combine :displaced-to with :initial-element or :initial-contents"
                    .to_string(),
            span: None,
        });
    }
    let displacement = displaced_array_arguments(
        "make-array",
        &dimensions,
        displaced_to,
        displaced_index_offset,
    )?;
    let logical_length = dimensions[0];
    let (displaced_to, displaced_index_offset, storage, elements) =
        if let Some(displacement) = displacement {
            (
                displacement.displaced_to,
                displacement.displaced_index_offset,
                Some(displacement.storage),
                None,
            )
        } else if let Some(contents) = initial_contents {
            let mut elements = Vec::with_capacity(total_size);
            flatten_array_contents("make-array", &contents, &dimensions, &mut elements)?;
            (None, 0, None, Some(elements))
        } else {
            (
                None,
                0,
                None,
                Some(vec![initial_element.unwrap_or(Value::Nil); total_size]),
            )
        };
    let element_type = element_type.unwrap_or_else(|| Value::symbol("T"));
    if dimensions.len() == 1 {
        let fill_pointer = fill_pointer
            .map(|value| array_fill_pointer("make-array", &value, logical_length))
            .transpose()?;
        Ok(if let Some(storage) = storage {
            Value::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
                storage,
                logical_length,
                fill_pointer,
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            )
        } else {
            Value::vector_with_fill_pointer_element_type_adjustable_and_displacement(
                elements.expect("non-displaced vector elements"),
                fill_pointer,
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            )
        })
    } else {
        if fill_pointer.is_some() {
            return Err(RuntimeError::InvalidForm {
                message: "make-array :fill-pointer requires a one-dimensional array".to_string(),
                span: None,
            });
        }
        Ok(if let Some(storage) = storage {
            Value::array_with_storage_element_type_adjustable_and_displacement(
                dimensions,
                storage,
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            )
        } else {
            Value::array_with_element_type_adjustable_and_displacement(
                dimensions,
                elements.expect("non-displaced array elements"),
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            )
        })
    }
}

fn adjust_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity(
            "adjust-array",
            "array, dimensions, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let source = &arguments[0];
    dimensions_for_array(source).ok_or_else(|| type_error("adjust-array", "array", source))?;
    let dimensions = parse_array_dimensions("adjust-array", &arguments[1])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut fill_pointer = None;
    let mut element_type = None;
    let mut displaced_to = None;
    let mut displaced_index_offset = None;
    if !(arguments.len() - 2).is_multiple_of(2) {
        return Err(arity(
            "adjust-array",
            "array, dimensions, and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[2..].chunks_exact(2) {
        let name = array_option_name("adjust-array", &pair[0])?;
        match name.as_str() {
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
            "FILL-POINTER" => fill_pointer = Some(pair[1].clone()),
            "ELEMENT-TYPE" => element_type = Some(pair[1].clone()),
            "DISPLACED-TO" => displaced_to = Some(pair[1].clone()),
            "DISPLACED-INDEX-OFFSET" => displaced_index_offset = Some(pair[1].clone()),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("adjust-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("adjust-array", &dimensions)?;
    if displaced_to.is_some() && (initial_element.is_some() || initial_contents.is_some()) {
        return Err(RuntimeError::InvalidForm {
            message:
                "adjust-array cannot combine :displaced-to with :initial-element or :initial-contents"
                    .to_string(),
            span: None,
        });
    }
    let displacement = displaced_array_arguments(
        "adjust-array",
        &dimensions,
        displaced_to,
        displaced_index_offset,
    )?;
    let logical_length = dimensions[0];
    let (displaced_to, displaced_index_offset, storage, elements) =
        if let Some(displacement) = displacement {
            (
                displacement.displaced_to,
                displacement.displaced_index_offset,
                Some(displacement.storage),
                None,
            )
        } else if let Some(contents) = initial_contents {
            let mut elements = Vec::with_capacity(total_size);
            flatten_array_contents("adjust-array", &contents, &dimensions, &mut elements)?;
            (None, 0, None, Some(elements))
        } else {
            let mut elements = array_elements(source).expect("array values carry elements");
            elements.truncate(total_size);
            if elements.len() < total_size {
                elements.resize(total_size, initial_element.unwrap_or(Value::Nil));
            }
            (None, 0, None, Some(elements))
        };
    let element_type = element_type.unwrap_or_else(|| {
        source
            .array_element_type_value()
            .expect("array values carry element type")
    });
    if dimensions.len() == 1 {
        let fill_pointer = if let Some(value) = fill_pointer {
            Some(array_fill_pointer("adjust-array", &value, logical_length)?)
        } else if let Some(existing) = source.vector_fill_pointer() {
            Some(array_fill_pointer(
                "adjust-array",
                &Value::Integer(existing as i64),
                logical_length,
            )?)
        } else {
            None
        };
        Ok(if let Some(storage) = storage {
            Value::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
                storage,
                logical_length,
                fill_pointer,
                element_type,
                source.is_adjustable_array(),
                displaced_to,
                displaced_index_offset,
            )
        } else {
            Value::vector_with_fill_pointer_element_type_adjustable_and_displacement(
                elements.expect("non-displaced vector elements"),
                fill_pointer,
                element_type,
                source.is_adjustable_array(),
                displaced_to,
                displaced_index_offset,
            )
        })
    } else {
        if fill_pointer.is_some() {
            return Err(RuntimeError::InvalidForm {
                message: "adjust-array :fill-pointer requires a one-dimensional array".to_string(),
                span: None,
            });
        }
        Ok(if let Some(storage) = storage {
            Value::array_with_storage_element_type_adjustable_and_displacement(
                dimensions,
                storage,
                element_type,
                source.is_adjustable_array(),
                displaced_to,
                displaced_index_offset,
            )
        } else {
            Value::array_with_element_type_adjustable_and_displacement(
                dimensions,
                elements.expect("non-displaced array elements"),
                element_type,
                source.is_adjustable_array(),
                displaced_to,
                displaced_index_offset,
            )
        })
    }
}

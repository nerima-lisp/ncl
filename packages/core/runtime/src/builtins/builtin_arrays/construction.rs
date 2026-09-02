use crate::builtins::{
    arity, array_elements, array_option_name, array_total_size_for, dimensions_for_array,
    flatten_array_contents, parse_array_dimensions,
};
use crate::builtins::index_argument;
use crate::{RuntimeError, Value};

#[expect(clippy::unnecessary_wraps)]
pub fn vector(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::vector(arguments.to_vec()))
}

pub fn fill_pointer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 {
        return Err(arity("fill-pointer", "one", arguments.len()));
    }
    let pointer = arguments[0]
        .vector_fill_pointer()
        .flatten()
        .ok_or_else(|| crate::builtins::type_error("fill-pointer", "vector with a fill pointer", &arguments[0]))?;
    crate::builtins::integer_from_usize("fill-pointer", pointer)
}

pub fn vector_push(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 2 {
        return Err(arity("vector-push", "two", arguments.len()));
    }
    let vector = &arguments[1];
    let pointer = vector_fill_pointer_for("vector-push", vector)?;
    let length = vector_items_length("vector-push", vector)?;
    if pointer >= length {
        return Ok(Value::Nil);
    }
    vector.set_vector_item(pointer, arguments[0].clone());
    vector.set_vector_fill_pointer(Some(pointer + 1));
    crate::builtins::integer_from_usize("vector-push", pointer)
}

pub fn vector_pop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 {
        return Err(arity("vector-pop", "one", arguments.len()));
    }
    let vector = &arguments[0];
    let pointer = vector_fill_pointer_for("vector-pop", vector)?;
    if pointer == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "vector-pop called on an empty vector".to_string(),
            span: None,
        });
    }
    let index = pointer - 1;
    let value = vector.vector_items().expect("validated vector")[index].clone();
    vector.set_vector_fill_pointer(Some(index));
    Ok(value)
}

pub fn vector_push_extend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("vector-push-extend", "two or three", arguments.len()));
    }
    let vector = &arguments[1];
    let pointer = vector_fill_pointer_for("vector-push-extend", vector)?;
    let mut length = vector_items_length("vector-push-extend", vector)?;
    if pointer >= length {
        if vector.vector_adjustable() != Some(true) {
            return Err(crate::builtins::type_error("vector-push-extend", "adjustable vector", vector));
        }
        if vector.is_displaced() {
            return Err(RuntimeError::InvalidForm {
                message: "vector-push-extend cannot extend a displaced vector".to_string(),
                span: None,
            });
        }
        let extension = arguments.get(2).map(|value| index_argument("vector-push-extend", value)).transpose()?.unwrap_or(1).max(1);
        if let Value::Vector(items) = vector {
            length = length.checked_add(extension).ok_or_else(|| crate::RuntimeError::InvalidForm { message: "vector-push-extend length overflow".to_string(), span: None })?;
            items.borrow_mut().resize(length, Value::Nil);
        }
    }
    vector.set_vector_item(pointer, arguments[0].clone());
    vector.set_vector_fill_pointer(Some(pointer + 1));
    crate::builtins::integer_from_usize("vector-push-extend", pointer)
}

fn vector_fill_pointer_for(function: &str, vector: &Value) -> Result<usize, RuntimeError> {
    vector.vector_fill_pointer().flatten().ok_or_else(|| crate::builtins::type_error(function, "vector with a fill pointer", vector))
}

fn vector_items_length(function: &str, vector: &Value) -> Result<usize, RuntimeError> {
    match vector {
        Value::Vector(items) => Ok(items.borrow().len()),
        _ => Err(crate::builtins::type_error(function, "vector", vector)),
    }
}

pub fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut element_type = Value::symbol("T");
    let mut adjustable = false;
    let mut fill_pointer = None;
    let mut displaced_to = None;
    let mut displaced_index_offset = 0;
    if !(arguments.len() - 1).is_multiple_of(2) {
        return Err(arity(
            "make-array",
            "one dimension and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[1..].as_chunks::<2>().0 {
        let name = array_option_name("make-array", &pair[0])?;
        match name.as_str() {
            "ELEMENT-TYPE" => element_type = pair[1].clone(),
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
            "ADJUSTABLE" => adjustable = pair[1].is_truthy(),
            "FILL-POINTER" => fill_pointer = Some(index_argument("make-array", &pair[1])?),
            "DISPLACED-TO" => displaced_to = Some(pair[1].clone()),
            "DISPLACED-INDEX-OFFSET" => displaced_index_offset = index_argument("make-array", &pair[1])?,
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("make-array", &dimensions)?;
    if fill_pointer.is_some() && dimensions.len() != 1 {
        return Err(crate::RuntimeError::InvalidForm {
            message: "make-array fill pointer requires a vector".to_string(),
            span: None,
        });
    }
    if displaced_to.is_some() && (initial_element.is_some() || initial_contents.is_some()) {
        return Err(crate::RuntimeError::InvalidForm { message: "make-array cannot combine displacement with initial contents".to_string(), span: None });
    }
    let displaced_storage = displaced_to.as_ref().and_then(Value::array_storage);
    if displaced_to.is_some() && displaced_storage.is_none() {
        return Err(crate::builtins::type_error("make-array", "array or vector", displaced_to.as_ref().unwrap()));
    }
    if let Some((storage, target_offset, _)) = displaced_storage.as_ref() {
        let end = target_offset.checked_add(displaced_index_offset).and_then(|offset| offset.checked_add(total_size)).ok_or_else(|| crate::RuntimeError::InvalidForm { message: "make-array displacement overflow".to_string(), span: None })?;
        if storage.borrow().len() < end { return Err(crate::RuntimeError::InvalidForm { message: "make-array displacement exceeds target".to_string(), span: None }); }
    }
    let elements = if let Some(contents) = initial_contents {
        let mut elements = Vec::with_capacity(total_size);
        flatten_array_contents("make-array", &contents, &dimensions, &mut elements)?;
        elements
    } else {
        vec![initial_element.unwrap_or(Value::Nil); total_size]
    };
    if dimensions.len() == 1 {
        let vector = if let Some((storage, target_offset, _)) = displaced_storage {
            Value::Vector(std::rc::Rc::new(crate::value::VectorData { elements: std::rc::Rc::new(std::cell::RefCell::new(elements)), metadata: std::cell::RefCell::new(crate::value::ArrayMetadata { element_type: element_type.clone(), adjustable, fill_pointer: None, displaced_to: Some(storage), displaced_to_value: displaced_to.clone(), displaced_index_offset: target_offset + displaced_index_offset }) }))
        } else { Value::vector(elements) };
        vector.set_array_element_type(element_type.clone());
        vector.set_vector_adjustable(adjustable);
        if let Some(fill_pointer) = fill_pointer {
            if fill_pointer > total_size {
                return Err(crate::RuntimeError::InvalidForm {
                    message: "make-array fill pointer exceeds vector length".to_string(),
                    span: None,
                });
            }
            vector.set_vector_fill_pointer(Some(fill_pointer));
        }
        Ok(vector)
    } else {
        let array = Value::array(dimensions, elements);
        array.set_array_element_type(element_type);
        array.set_array_adjustable(adjustable);
        Ok(array)
    }
}

pub fn adjust_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("adjust-array", "array and dimensions", arguments.len()));
    }
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| crate::builtins::type_error("adjust-array", "array", &arguments[0]))?;
    let dimensions = parse_array_dimensions("adjust-array", &arguments[1])?;
    let total_size = array_total_size_for("adjust-array", &dimensions)?;
    if !(arguments.len() - 2).is_multiple_of(2) {
        return Err(arity("adjust-array", "array, dimensions, and keyword/value pairs", arguments.len()));
    }
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut fill_pointer = None;
    let mut adjustable = None;
    for pair in arguments[2..].as_chunks::<2>().0 {
        let name = array_option_name("adjust-array", &pair[0])?;
        match name.as_str() {
            "INITIAL-ELEMENT" if initial_contents.is_none() => initial_element = Some(pair[1].clone()),
            "INITIAL-CONTENTS" if initial_element.is_none() => initial_contents = Some(pair[1].clone()),
            "INITIAL-ELEMENT" | "INITIAL-CONTENTS" => return Err(crate::RuntimeError::InvalidForm {
                message: "adjust-array cannot combine :initial-element and :initial-contents".to_string(), span: None,
            }),
            "FILL-POINTER" => fill_pointer = Some(index_argument("adjust-array", &pair[1])?),
            "ADJUSTABLE" => adjustable = Some(pair[1].is_truthy()),
            _ => return Err(crate::RuntimeError::InvalidForm {
                message: format!("adjust-array does not support keyword :{name}"), span: None,
            }),
        }
    }
    let mut elements = if let Some(contents) = initial_contents.as_ref() {
        let mut values = Vec::with_capacity(total_size);
        flatten_array_contents("adjust-array", &contents, &dimensions, &mut values)?;
        values
    } else {
        vec![initial_element.clone().unwrap_or(Value::Nil); total_size]
    };
    if initial_element.is_none() && initial_contents.is_none() {
        if let Some(old_elements) = array_elements(&arguments[0]) {
            for (target, source) in elements.iter_mut().zip(old_elements).take(total_size) {
                *target = source;
            }
        }
    }
    if dimensions.len() == 1 {
        if let Some(fill_pointer) = fill_pointer {
            if fill_pointer > total_size {
                return Err(crate::RuntimeError::InvalidForm {
                    message: "adjust-array fill pointer exceeds vector length".to_string(),
                    span: None,
                });
            }
        }
        let adjustable_value = adjustable.unwrap_or(arguments[0].vector_adjustable().unwrap_or(false));
        if arguments[0].vector_adjustable() == Some(true)
            && adjustable != Some(false)
            && !arguments[0].is_displaced()
        {
            let fill_pointer = fill_pointer.or_else(|| arguments[0].vector_fill_pointer().flatten());
            if let Value::Vector(items) = &arguments[0] {
                *items.borrow_mut() = elements;
            }
            arguments[0].set_vector_fill_pointer(fill_pointer.map(|value| value.min(total_size)));
            return Ok(arguments[0].clone());
        }
        let vector = Value::vector(elements);
        vector.set_array_element_type(
            arguments[0]
                .array_element_type()
                .unwrap_or_else(|| Value::symbol("T")),
        );
        vector.set_vector_adjustable(adjustable_value);
        vector.set_vector_fill_pointer(fill_pointer.or_else(|| arguments[0].vector_fill_pointer().flatten()).map(|value| value.min(total_size)));
        Ok(vector)
    } else {
        let array = Value::array(dimensions, elements);
        array.set_array_element_type(
            arguments[0]
                .array_element_type()
                .unwrap_or_else(|| Value::symbol("T")),
        );
        array.set_array_adjustable(adjustable.unwrap_or(arguments[0].array_adjustable().unwrap_or(false)));
        if fill_pointer.is_some() {
            return Err(crate::RuntimeError::InvalidForm {
                message: "adjust-array fill pointer requires a vector".to_string(),
                span: None,
            });
        }
        Ok(array)
    }
}

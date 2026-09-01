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

pub fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut adjustable = false;
    let mut fill_pointer = None;
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
    let elements = if let Some(contents) = initial_contents {
        let mut elements = Vec::with_capacity(total_size);
        flatten_array_contents("make-array", &contents, &dimensions, &mut elements)?;
        elements
    } else {
        vec![initial_element.unwrap_or(Value::Nil); total_size]
    };
    if dimensions.len() == 1 {
        let vector = Value::vector(elements);
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
    for pair in arguments[2..].as_chunks::<2>().0 {
        let name = array_option_name("adjust-array", &pair[0])?;
        match name.as_str() {
            "INITIAL-ELEMENT" if initial_contents.is_none() => initial_element = Some(pair[1].clone()),
            "INITIAL-CONTENTS" if initial_element.is_none() => initial_contents = Some(pair[1].clone()),
            "INITIAL-ELEMENT" | "INITIAL-CONTENTS" => return Err(crate::RuntimeError::InvalidForm {
                message: "adjust-array cannot combine :initial-element and :initial-contents".to_string(), span: None,
            }),
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
        if arguments[0].vector_adjustable() == Some(true) {
            let fill_pointer = arguments[0].vector_fill_pointer().flatten();
            if let Value::Vector(items) = &arguments[0] {
                *items.borrow_mut() = elements;
            }
            arguments[0].set_vector_fill_pointer(fill_pointer.map(|value| value.min(total_size)));
            return Ok(arguments[0].clone());
        }
        Ok(Value::vector(elements))
    } else {
        Ok(Value::array(dimensions, elements))
    }
}

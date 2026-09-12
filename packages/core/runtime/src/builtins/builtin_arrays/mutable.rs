use super::construction::{make_array_from_dimensions, parse_options_for_adjust_array};
use crate::builtins::{
    arity, exact, index_argument, integer_from_usize, parse_array_dimensions, type_error,
};
use crate::{RuntimeError, Value};

fn vector_storage(function: &str, value: &Value) -> Result<crate::SharedElements, RuntimeError> {
    match value {
        Value::Vector(elements) => Ok(elements.clone()),
        other => Err(type_error(function, "vector", other)),
    }
}

pub fn fill_pointer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "fill-pointer", 1)?;
    let elements = vector_storage("fill-pointer", &arguments[0])?;
    let Some(pointer) = elements.fill_pointer() else {
        return Err(type_error(
            "fill-pointer",
            "vector with a fill pointer",
            &arguments[0],
        ));
    };
    integer_from_usize("fill-pointer", pointer)
}

pub fn array_has_fill_pointer_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-has-fill-pointer-p", 1)?;
    let Some(elements) = arguments[0].array_storage() else {
        return Err(type_error(
            "array-has-fill-pointer-p",
            "array",
            &arguments[0],
        ));
    };
    Ok(Value::boolean(elements.has_fill_pointer()))
}

pub fn vector_push(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vector-push", 2)?;
    let elements = vector_storage("vector-push", &arguments[1])?;
    if !elements.has_fill_pointer() {
        return Err(type_error(
            "vector-push",
            "vector with a fill pointer",
            &arguments[1],
        ));
    }
    elements
        .push_value(arguments[0].clone())
        .map_or(Ok(Value::Nil), |index| {
            integer_from_usize("vector-push", index)
        })
}

pub fn vector_push_extend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("vector-push-extend", "two or three", arguments.len()));
    }
    let elements = vector_storage("vector-push-extend", &arguments[1])?;
    if !elements.has_fill_pointer() {
        return Err(type_error(
            "vector-push-extend",
            "vector with a fill pointer",
            &arguments[1],
        ));
    }
    if let Some(index) = elements.push_value(arguments[0].clone()) {
        return integer_from_usize("vector-push-extend", index);
    }
    let extension = arguments
        .get(2)
        .map_or(Ok(1), |value| index_argument("vector-push-extend", value))?;
    if !elements.grow(extension) {
        return Err(RuntimeError::InvalidForm {
            message: "vector-push-extend requires an adjustable vector with capacity to grow"
                .to_owned(),
            span: None,
        });
    }
    elements
        .push_value(arguments[0].clone())
        .map_or(Ok(Value::Nil), |index| {
            integer_from_usize("vector-push-extend", index)
        })
}

pub fn vector_pop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vector-pop", 1)?;
    let elements = vector_storage("vector-pop", &arguments[0])?;
    if !elements.has_fill_pointer() {
        return Err(type_error(
            "vector-pop",
            "vector with a fill pointer",
            &arguments[0],
        ));
    }
    Ok(elements.pop_value().unwrap_or(Value::Nil))
}

pub fn adjust_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("adjust-array", "at least two", arguments.len()));
    }
    let old = arguments[0].clone();
    let old_elements = old
        .array_storage()
        .ok_or_else(|| type_error("adjust-array", "array", &old))?;
    let dimensions = parse_array_dimensions("adjust-array", &arguments[1])?;
    let mut options = parse_options_for_adjust_array(&arguments[2..])?;
    if options.element_type.is_none() {
        options.element_type = Some(old_elements.element_type());
    }
    if !options.fill_pointer_specified {
        options.fill_pointer = old_elements
            .fill_pointer()
            .map(|pointer| Value::big_integer(pointer.into()));
    }
    if !options.adjustable_specified {
        options.adjustable = old_elements.is_adjustable();
    }
    let preserve_contents = options.initial_contents.is_none() && options.displaced_to.is_none();
    let adjusted = make_array_from_dimensions("adjust-array", dimensions, options)?;
    if preserve_contents {
        let new_elements = adjusted
            .array_storage()
            .expect("make-array must produce an array");
        let values = old_elements.snapshot();
        let count = values.len().min(new_elements.len());
        if !new_elements.replace_range(0, &values[..count]) {
            return Err(RuntimeError::InvalidForm {
                message: "adjust-array could not preserve array contents".to_owned(),
                span: None,
            });
        }
    }
    Ok(adjusted)
}

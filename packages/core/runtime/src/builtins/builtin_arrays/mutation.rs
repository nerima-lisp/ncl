use crate::builtins::arity;
use crate::{RuntimeError, Value};

pub fn fill_pointer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 {
        return Err(arity("fill-pointer", "one", arguments.len()));
    }
    let Some(elements) = arguments[0].array_storage() else {
        return Err(crate::builtins::type_error(
            "fill-pointer",
            "vector with a fill pointer",
            &arguments[0],
        ));
    };
    Ok(Value::Integer(elements.fill_pointer().unwrap_or(0) as i64))
}

pub fn vector_push(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 2 {
        return Err(arity("vector-push", "two", arguments.len()));
    }
    let Some(elements) = arguments[1].array_storage() else {
        return Err(crate::builtins::type_error(
            "vector-push",
            "vector with a fill pointer",
            &arguments[1],
        ));
    };
    let Some(index) = elements.fill_pointer() else { return Ok(Value::Nil); };
    if index == elements.len() {
        return Ok(Value::Nil);
    }
    if !elements.set(index, arguments[0].clone()) { return Ok(Value::Nil); }
    let _ = elements.set_fill_pointer(index + 1);
    Ok(Value::Integer(index as i64))
}

pub fn vector_pop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 {
        return Err(arity("vector-pop", "one", arguments.len()));
    }
    let Some(elements) = arguments[0].array_storage() else {
        return Err(crate::builtins::type_error(
            "vector-pop",
            "vector with a fill pointer",
            &arguments[0],
        ));
    };
    let Some(fill_pointer) = elements.fill_pointer() else { return Ok(Value::Nil); };
    if fill_pointer == 0 {
        return Ok(Value::Nil);
    }
    let index = fill_pointer - 1;
    let value = elements.get(index).unwrap_or(Value::Nil);
    let _ = elements.set_fill_pointer(index);
    Ok(value)
}

pub fn vector_push_extend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("vector-push-extend", "two or three", arguments.len()));
    }
    let Some(elements) = arguments[1].array_storage() else {
        return Err(crate::builtins::type_error(
            "vector-push-extend",
            "vector with a fill pointer",
            &arguments[1],
        ));
    };
    let Some(fill_pointer) = elements.fill_pointer() else { return Ok(Value::Nil); };
    if fill_pointer == elements.len() {
        if !elements.is_adjustable() {
            return Ok(Value::Nil);
        }
        let extension = match arguments.get(2) {
            None => 1,
            Some(Value::Integer(value)) if *value > 0 => *value as usize,
            Some(value) => {
                return Err(crate::builtins::type_error(
                    "vector-push-extend",
                    "positive integer extension",
                    value,
                ));
            }
        };
        let _ = extension;
        return Ok(Value::Nil);
    }
    let index = fill_pointer;
    if !elements.set(index, arguments[0].clone()) { return Ok(Value::Nil); }
    let _ = elements.set_fill_pointer(index + 1);
    Ok(Value::Integer(index as i64))
}

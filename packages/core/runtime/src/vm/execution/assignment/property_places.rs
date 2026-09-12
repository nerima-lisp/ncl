#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn execute_setf_gethash_place(
    place: &Form,
    stack: &mut Vec<Value>,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("setf gethash place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf gethash place has no value", span))?
        .primary_value();
    let table = stack
        .pop()
        .ok_or_else(|| invalid("setf gethash place has no table", span))?;
    let key = stack
        .pop()
        .ok_or_else(|| invalid("setf gethash place has no key", span))?;
    Runtime::set_gethash_place_value(&table, &key, value.clone(), place.span)?;
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_setf_getf_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("setf getf place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf getf place has no value", span))?
        .primary_value();
    let indicator = stack
        .pop()
        .ok_or_else(|| invalid("setf getf place has no indicator", span))?;
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf getf place has no plist", span))?;
    runtime.set_getf_place_value(
        place,
        &current,
        &indicator,
        value.clone(),
        environment,
        span,
    )?;
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_push_getf_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("push getf place has insufficient values", span));
    }
    let indicator = stack
        .pop()
        .ok_or_else(|| invalid("push getf place has no indicator", span))?;
    let plist = stack
        .pop()
        .ok_or_else(|| invalid("push getf place has no plist", span))?;
    let item = stack
        .pop()
        .ok_or_else(|| invalid("push getf place has no item", span))?
        .primary_value();
    let Some(items) = plist.list_items() else {
        return Err(RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: plist.type_name().to_string(),
            span: Some(span),
        });
    };
    if !items.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "getf requires an even-length property list".to_string(),
            span: Some(span),
        });
    }
    let current = items
        .as_chunks::<2>()
        .0
        .iter()
        .find(|pair| indicator.eq_value(&pair[0]))
        .map_or(Value::Nil, |pair| pair[1].clone());
    let value = crate::builtins::cons(&[item, current])?;
    runtime.set_getf_place_value(place, &plist, &indicator, value.clone(), environment, span)?;
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_pushnew_getf_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("pushnew getf place has insufficient values", span));
    }
    let indicator = stack
        .pop()
        .ok_or_else(|| invalid("pushnew getf place has no indicator", span))?;
    let plist = stack
        .pop()
        .ok_or_else(|| invalid("pushnew getf place has no plist", span))?;
    let item = stack
        .pop()
        .ok_or_else(|| invalid("pushnew getf place has no item", span))?
        .primary_value();
    let Some(items) = plist.list_items() else {
        return Err(RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: plist.type_name().to_string(),
            span: Some(span),
        });
    };
    if !items.len().is_multiple_of(2) {
        return Err(RuntimeError::InvalidForm {
            message: "getf requires an even-length property list".to_string(),
            span: Some(span),
        });
    }
    let current = items
        .as_chunks::<2>()
        .0
        .iter()
        .find(|pair| indicator.eq_value(&pair[0]))
        .map_or(Value::Nil, |pair| pair[1].clone());
    let value = if let Some(current_items) = current.list_items() {
        if current_items.iter().any(|entry| entry.eq_value(&item)) {
            current
        } else {
            crate::builtins::cons(&[item, current])?
        }
    } else {
        return Err(RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: current.type_name().to_string(),
            span: Some(span),
        });
    };
    runtime.set_getf_place_value(place, &plist, &indicator, value.clone(), environment, span)?;
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

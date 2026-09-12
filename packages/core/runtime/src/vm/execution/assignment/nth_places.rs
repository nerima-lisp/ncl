use super::*;
use ncl_syntax::Form;

pub(super) fn execute_setf_nth_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("setf nth place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf nth place has no value", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf nth place has no list", span))?;
    let index = stack
        .last()
        .ok_or_else(|| invalid("setf nth place has no index", span))?;
    runtime.set_nth_place_value(place, index, &current, value.clone(), environment, span)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("setf nth place has no result", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_pop_nth_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 2 {
        return Err(invalid("pop nth place has insufficient values", span));
    }
    let current = stack
        .pop()
        .ok_or_else(|| invalid("pop nth place has no list", span))?;
    let index = stack
        .pop()
        .ok_or_else(|| invalid("pop nth place has no index", span))?;
    let value = runtime.pop_nth_place_value(place, &index, &current, environment, span)?;
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_push_nth_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("push nth place has insufficient values", span));
    }
    let current = stack
        .pop()
        .ok_or_else(|| invalid("push nth place has no list", span))?;
    let index = stack
        .pop()
        .ok_or_else(|| invalid("push nth place has no index", span))?;
    let item = stack
        .pop()
        .ok_or_else(|| invalid("push nth place has no item", span))?;
    let value = runtime.push_nth_place_value(place, &index, &current, item, environment, span)?;
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_pushnew_nth_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("pushnew nth place has insufficient values", span));
    }
    let current = stack
        .pop()
        .ok_or_else(|| invalid("pushnew nth place has no list", span))?;
    let index = stack
        .pop()
        .ok_or_else(|| invalid("pushnew nth place has no index", span))?;
    let item = stack
        .pop()
        .ok_or_else(|| invalid("pushnew nth place has no item", span))?;
    let value =
        runtime.pushnew_nth_place_value(place, &index, &current, item, environment, span)?;
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_map_into_nth_place(
    runtime: &Runtime,
    sequence_count: usize,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let required = sequence_count.saturating_add(3);
    if stack.len() < required {
        return Err(invalid("MAP-INTO NTH place has insufficient values", span));
    }
    let start = stack.len() - required;
    let index = stack[start].clone();
    let container = stack[start + 1].clone();
    let function = stack[start + 2].clone();
    let sequences = stack[start + 3..].to_vec();
    let index_number = Runtime::setf_index(index.clone(), span)?;
    let destination = container
        .list_items()
        .and_then(|items| items.get(index_number).cloned())
        .ok_or_else(|| invalid("MAP-INTO NTH index is out of bounds", span))?;
    let result =
        runtime.apply_sequence_map_into(&destination, &function, &sequences, environment, span)?;
    runtime.set_nth_place_value(place, &index, &container, result.clone(), environment, span)?;
    stack.truncate(start);
    stack.push(result);
    *program_counter += 1;
    Ok(true)
}

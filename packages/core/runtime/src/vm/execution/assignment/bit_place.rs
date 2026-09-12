use super::super::{Environment, Runtime, RuntimeError, Span, Value, invalid};
use ncl_syntax::Form;

pub(super) fn execute_setf_bit_place(
    runtime: &Runtime,
    index_count: usize,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if index_count == 0 || stack.len() < index_count + 2 {
        return Err(invalid("setf bit place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf bit place has no value", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf bit place has no array", span))?;
    let start = stack.len() - index_count;
    let indices = stack[start..].to_vec();
    runtime.set_bit_place_value(place, &indices, &current, value.clone(), environment)?;
    stack.truncate(start);
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

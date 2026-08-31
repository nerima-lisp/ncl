#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute_rotatef(
    runtime: &Runtime,
    places: &[(String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() {
        return Err(invalid("rotatef has too few values on the stack", span));
    }
    let values = stack.split_off(stack.len() - places.len());
    for (index, (name, escaped)) in places.iter().enumerate() {
        let value = values[(index + values.len() - 1) % values.len()]
            .clone()
            .primary_value();
        if *escaped {
            runtime.set_or_define_exact_in(name, value, environment, span)?;
        } else {
            runtime.set_or_define_in(name, value, environment, span)?;
        }
    }
    stack.push(Value::Nil);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_shiftf(
    runtime: &Runtime,
    places: &[(String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() + 1 {
        return Err(invalid("shiftf has too few values on the stack", span));
    }
    let values = stack.split_off(stack.len() - places.len() - 1);
    let old_first = values[0].clone().primary_value();
    for (index, (name, escaped)) in places.iter().enumerate() {
        let value = values
            .get(index + 1)
            .cloned()
            .unwrap_or_else(|| Value::Nil)
            .primary_value();
        if *escaped {
            runtime.set_or_define_exact_in(name, value, environment, span)?;
        } else {
            runtime.set_or_define_in(name, value, environment, span)?;
        }
    }
    stack.push(old_first);
    *program_counter += 1;
    Ok(true)
}

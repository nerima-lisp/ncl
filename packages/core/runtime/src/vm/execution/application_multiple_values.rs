#![allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_multiple_value_call_instruction(
    runtime: &Runtime,
    value_form_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < value_form_count.saturating_add(1) {
        return Err(invalid(
            "multiple-value-call has too few stack values",
            span,
        ));
    }
    let start = stack.len() - value_form_count.saturating_add(1);
    let mut operands = stack.split_off(start);
    let function_value = operands
        .first()
        .cloned()
        .ok_or_else(|| invalid("multiple-value-call has no function value", span))?;
    let mut arguments = Vec::new();
    for value in operands.drain(1..) {
        arguments.extend(value.multiple_values());
    }
    stack.push(runtime.apply_in(
        &function_value.primary_value(),
        &arguments,
        span,
        environment,
    )?);
    Ok(())
}

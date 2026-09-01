#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_call_instruction(
    runtime: &Runtime,
    argument_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count.saturating_add(1) {
        return Err(invalid("call has too few stack values", span));
    }
    let arguments_start = stack.len() - argument_count;
    let arguments = stack.split_off(arguments_start);
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("call has no function value", span))?;
    let arguments = arguments
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    stack.push(runtime.apply_in(
        &function_value.primary_value(),
        &arguments,
        span,
        environment,
    )?);
    Ok(())
}

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

pub fn execute_apply_instruction(
    runtime: &Runtime,
    argument_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if argument_count == 0 || stack.len() < argument_count.saturating_add(1) {
        return Err(invalid("apply has too few stack values", span));
    }
    let arguments_start = stack.len() - argument_count;
    let mut evaluated = stack.split_off(arguments_start);
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("apply has no function value", span))?;
    let final_value = evaluated
        .pop()
        .ok_or_else(|| invalid("apply has no final list", span))?;
    let mut arguments = evaluated
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let mut final_arguments = final_value
        .primary_value()
        .list_items()
        .ok_or_else(|| invalid("apply's final argument must be a proper list", span))?;
    arguments.append(&mut final_arguments);
    stack.push(runtime.apply_in(
        &function_value.primary_value(),
        &arguments,
        span,
        environment,
    )?);
    Ok(())
}

pub fn execute_list_mapping_instruction(
    runtime: &Runtime,
    operation: &str,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if sequence_count == 0 || stack.len() < sequence_count.saturating_add(1) {
        return Err(invalid(
            &format!(
                "{} has too few stack values",
                operation.to_ascii_lowercase()
            ),
            span,
        ));
    }
    let sequences_start = stack.len() - sequence_count;
    let sequences = stack.split_off(sequences_start);
    let function_value = stack.pop().ok_or_else(|| {
        invalid(
            &format!("{} has no function value", operation.to_ascii_lowercase()),
            span,
        )
    })?;
    let result = runtime.apply_list_mapping(
        operation,
        &function_value.primary_value(),
        &sequences,
        environment,
        span,
    )?;
    stack.push(result);
    Ok(())
}

pub fn execute_sequence_quantifier_instruction(
    runtime: &Runtime,
    operation: &str,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if sequence_count == 0 || stack.len() < sequence_count.saturating_add(1) {
        return Err(invalid(
            &format!("{} has too few stack values", operation.to_ascii_lowercase()),
            span,
        ));
    }
    let sequences_start = stack.len() - sequence_count;
    let sequences = stack.split_off(sequences_start);
    let predicate = stack.pop().ok_or_else(|| {
        invalid(
            &format!("{} has no predicate value", operation.to_ascii_lowercase()),
            span,
        )
    })?;
    stack.push(runtime.apply_sequence_quantifier(
        operation,
        &predicate.primary_value(),
        &sequences,
        environment,
        span,
    )?);
    Ok(())
}

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

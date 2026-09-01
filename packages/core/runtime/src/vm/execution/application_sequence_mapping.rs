#[allow(clippy::wildcard_imports)]
use super::*;

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
            &format!(
                "{} has too few stack values",
                operation.to_ascii_lowercase()
            ),
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

pub fn execute_sequence_mapping_instruction(
    runtime: &Runtime,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < sequence_count.saturating_add(2) {
        return Err(invalid("map has too few stack values", span));
    }
    let sequences_start = stack.len() - sequence_count;
    let sequences = stack.split_off(sequences_start);
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("map has no function value", span))?;
    let result_type = stack
        .pop()
        .ok_or_else(|| invalid("map has no result type", span))?;
    stack.push(runtime.apply_sequence_mapping(
        &result_type.primary_value(),
        &function_value.primary_value(),
        &sequences,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_map_into_instruction(
    runtime: &Runtime,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < sequence_count.saturating_add(2) {
        return Err(invalid("map-into has too few stack values", span));
    }
    let sequences_start = stack.len() - sequence_count;
    let sequences = stack.split_off(sequences_start);
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("map-into has no function value", span))?;
    let destination = stack
        .pop()
        .ok_or_else(|| invalid("map-into has no destination value", span))?;
    stack.push(runtime.apply_sequence_map_into(
        &destination.primary_value(),
        &function_value.primary_value(),
        &sequences,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_reduce_instruction(
    runtime: &Runtime,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("reduce has too few stack values", span));
    }
    let options_start = stack.len() - option_count;
    let options = stack.split_off(options_start);
    let sequence = stack
        .pop()
        .ok_or_else(|| invalid("reduce has no sequence value", span))?;
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("reduce has no function value", span))?;
    stack.push(runtime.apply_sequence_reduce(
        &function_value.primary_value(),
        &sequence.primary_value(),
        &options,
        environment,
        span,
    )?);
    Ok(())
}


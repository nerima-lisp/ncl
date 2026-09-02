#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_class_introspection_instruction(
    _runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "class introspection operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value =
        Runtime::apply_class_introspection_primitive(operation, &arguments, environment, span)
            .unwrap_or_else(|| Err(invalid("unknown class introspection operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_slot_operation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("slot operation has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime.apply_primitive(operation, &arguments, environment, span)?;
    stack.push(value);
    Ok(())
}

pub fn execute_condition_operation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "condition operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_condition_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown condition operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_restart_operation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("restart operation has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_restart_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown restart operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_method_operation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("method operation has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_method_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown method operation", span)))?;
    stack.push(value);
    Ok(())
}

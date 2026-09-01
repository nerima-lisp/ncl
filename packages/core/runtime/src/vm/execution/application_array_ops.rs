#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_vector_construction_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "vector construction has too few stack values",
            span,
        ));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::vector(&arguments)?);
    Ok(())
}

pub fn execute_array_construction_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("array construction has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::make_array(&arguments)?);
    Ok(())
}

pub fn execute_array_adjustment_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("array adjustment has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::adjust_array(&arguments)?);
    Ok(())
}


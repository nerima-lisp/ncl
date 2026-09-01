#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_sequence_length_instruction(
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("sequence-length has too few stack values", span))?;
    stack.push(crate::builtins::length(&[value])?);
    Ok(())
}

pub fn execute_sequence_element_instruction(
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let index = stack
        .pop()
        .ok_or_else(|| invalid("sequence-element has too few stack values", span))?;
    let sequence = stack
        .pop()
        .ok_or_else(|| invalid("sequence-element has too few stack values", span))?;
    stack.push(crate::builtins::elt(&[sequence, index])?);
    Ok(())
}

pub fn execute_sequence_subseq_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("sequence-subseq has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::subseq(&arguments)?);
    Ok(())
}

pub fn execute_sequence_mutation_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("sequence mutation has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let value = match operation {
        "FILL" => crate::builtins::fill(&arguments)?,
        "REPLACE" => crate::builtins::replace(&arguments)?,
        _ => return Err(invalid("unknown sequence mutation operation", span)),
    };
    stack.push(value);
    Ok(())
}

pub fn execute_sequence_concatenate_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "sequence-concatenate has too few stack values",
            span,
        ));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::concatenate(&arguments)?);
    Ok(())
}

pub fn execute_sequence_conversion_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "sequence conversion has too few stack values",
            span,
        ));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let value = match operation {
        "MAKE-SEQUENCE" => crate::builtins::make_sequence(&arguments)?,
        "COERCE" => crate::builtins::coerce(&arguments)?,
        _ => return Err(invalid("unknown sequence conversion operation", span)),
    };
    stack.push(value);
    Ok(())
}


#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::SetfArefDynamic {
            rank,
            operator,
            name,
            escaped,
        } => execute_aref(
            runtime,
            *rank,
            operator,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfBitDynamic {
            rank,
            name,
            escaped,
        } => execute_bit(
            runtime,
            *rank,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        _ => Ok(false),
    }
}

fn execute_aref(
    runtime: &Runtime,
    rank: usize,
    operator: &str,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf aref has no value on the stack", span))?
        .primary_value();
    if stack.len() < rank.saturating_add(1) {
        return Err(invalid("setf aref has an incomplete stack", span));
    }
    let indices = stack.split_off(stack.len() - rank);
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf aref has no target on the stack", span))?
        .primary_value();
    let indices = indices
        .iter()
        .map(|index| crate::builtins::index_argument("setf array accessor", index))
        .collect::<Result<Vec<_>, _>>()?;
    let updated = match &current {
        Value::Vector(_) => {
            if rank != 1 {
                return Err(invalid("setf aref requires one vector index", span));
            }
            current.set_vector_item(indices[0], value.clone())
                .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
            current.clone()
        }
        Value::Array { dimensions, .. } => {
            if operator == "SVREF" {
                return Err(RuntimeError::Type {
                    expected: "SIMPLE-VECTOR".to_string(),
                    actual: "ARRAY".to_string(),
                    span: Some(span),
                });
            }
            let offset = if operator == "ROW-MAJOR-AREF" {
                if rank != 1 {
                    return Err(invalid("setf row-major-aref requires one index", span));
                }
                indices[0]
            } else {
                array_offset(
                    dimensions,
                    rank,
                    &indices,
                    "setf aref has the wrong number of indices",
                    span,
                )?
            };
            current.set_array_item(offset, value.clone())
                .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
            current.clone()
        }
        other => {
            return Err(RuntimeError::Type {
                expected: "ARRAY or VECTOR".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            });
        }
    };
    store_array_value(
        runtime,
        name,
        escaped,
        updated,
        value,
        stack,
        environment,
        program_counter,
        span,
    )
}

fn execute_bit(
    runtime: &Runtime,
    rank: usize,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf bit has no value on the stack", span))?
        .primary_value();
    if stack.len() < rank.saturating_add(1) {
        return Err(invalid("setf bit has an incomplete stack", span));
    }
    let indices = stack.split_off(stack.len() - rank);
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf bit has no target on the stack", span))?
        .primary_value();
    let dimensions = match &current {
        Value::Vector(items) => vec![items.borrow().len()],
        Value::Array { dimensions, .. } => dimensions.as_ref().clone(),
        other => {
            return Err(RuntimeError::Type {
                expected: "ARRAY".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            });
        }
    };
    let indices = indices
        .iter()
        .map(|index| crate::builtins::index_argument("setf bit", index))
        .collect::<Result<Vec<_>, _>>()?;
    let offset = array_offset(
        &dimensions,
        rank,
        &indices,
        "setf bit has the wrong number of indices",
        span,
    )?;
    if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
        return Err(RuntimeError::Type {
            expected: "BIT".to_string(),
            actual: value.type_name().to_string(),
            span: Some(span),
        });
    }
    let updated = match &current {
        Value::Vector(_) => {
            current.set_vector_item(offset, value.clone()).ok_or_else(|| invalid("SETF index is out of bounds", span))?;
            current.clone()
        }
        Value::Array { .. } => {
            current.set_array_item(offset, value.clone()).ok_or_else(|| invalid("SETF index is out of bounds", span))?;
            current.clone()
        }
        _ => unreachable!(),
    };
    store_array_value(
        runtime,
        name,
        escaped,
        updated,
        value,
        stack,
        environment,
        program_counter,
        span,
    )
}

fn array_offset(
    dimensions: &[usize],
    rank: usize,
    indices: &[usize],
    rank_error: &str,
    span: Span,
) -> Result<usize, RuntimeError> {
    if dimensions.len() != rank {
        return Err(invalid(rank_error, span));
    }
    let mut offset = 0_usize;
    for (axis, (&dimension, &index)) in dimensions.iter().zip(indices).enumerate() {
        if index >= dimension {
            return Err(invalid("SETF index is out of bounds", span));
        }
        let stride = dimensions[axis + 1..]
            .iter()
            .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
            .ok_or_else(|| invalid("SETF index is too large", span))?;
        offset = offset
            .checked_add(
                index
                    .checked_mul(stride)
                    .ok_or_else(|| invalid("SETF index is too large", span))?,
            )
            .ok_or_else(|| invalid("SETF index is too large", span))?;
    }
    Ok(offset)
}

fn store_array_value(
    runtime: &Runtime,
    name: &str,
    escaped: bool,
    updated: Value,
    value: Value,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

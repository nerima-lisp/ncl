#[allow(clippy::wildcard_imports)]
use super::*;

mod list;
mod property;
mod sequence;
mod symbol_cell;

pub(super) fn execute_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if sequence::execute(
        runtime,
        instruction,
        stack,
        environment,
        program_counter,
        span,
    )? {
        return Ok(true);
    }
    if property::execute(
        runtime,
        instruction,
        stack,
        environment,
        program_counter,
        span,
    )? {
        return Ok(true);
    }
    match instruction {
        Instruction::SetfSymbolCellDynamic { operator } => {
            symbol_cell::execute(runtime, operator, stack, program_counter, span)
        }
        Instruction::Set(name) | Instruction::SetExact(name) => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("setq has no value on the stack", span))?
                .primary_value();
            if matches!(instruction, Instruction::Set(_)) {
                runtime.set_or_define_in(name, value.clone(), environment, span)?;
            } else {
                runtime.set_or_define_exact_in(name, value.clone(), environment, span)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfList {
            operator,
            name,
            escaped,
        } => list::execute(
            runtime,
            operator,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfArefDynamic {
            rank,
            operator,
            name,
            escaped,
        } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf aref has no value on the stack", span))?
                .primary_value();
            if stack.len() < rank.saturating_add(1) {
                return Err(invalid("setf aref has an incomplete stack", span));
            }
            let indices = stack.split_off(stack.len() - *rank);
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf aref has no target on the stack", span))?
                .primary_value();
            let indices = indices
                .iter()
                .map(|index| crate::builtins::index_argument("setf array accessor", index))
                .collect::<Result<Vec<_>, _>>()?;
            let updated = match current {
                Value::Vector(_) => {
                    if *rank != 1 {
                        return Err(invalid("setf aref requires one vector index", span));
                    }
                    let mut elements =
                        current.vector_items().ok_or_else(|| RuntimeError::Type {
                            expected: "VECTOR".to_string(),
                            actual: current.type_name().to_string(),
                            span: Some(span),
                        })?;
                    let slot = elements
                        .get_mut(indices[0])
                        .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
                    *slot = value.clone();
                    Value::vector(elements)
                }
                Value::Array { ref dimensions, .. } => {
                    if operator == "SVREF" {
                        return Err(RuntimeError::Type {
                            expected: "SIMPLE-VECTOR".to_string(),
                            actual: "ARRAY".to_string(),
                            span: Some(span),
                        });
                    }
                    if operator == "ROW-MAJOR-AREF" {
                        if *rank != 1 {
                            return Err(invalid("setf row-major-aref requires one index", span));
                        }
                        let mut elements =
                            current.array_items().ok_or_else(|| RuntimeError::Type {
                                expected: "ARRAY".to_string(),
                                actual: current.type_name().to_string(),
                                span: Some(span),
                            })?;
                        let slot = elements
                            .get_mut(indices[0])
                            .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
                        *slot = value.clone();
                        Value::array(dimensions.as_ref().clone(), elements)
                    } else {
                        if dimensions.len() != *rank {
                            return Err(invalid("setf aref has the wrong number of indices", span));
                        }
                        let mut offset = 0_usize;
                        for (axis, (&dimension, &index)) in
                            dimensions.iter().zip(&indices).enumerate()
                        {
                            if index >= dimension {
                                return Err(invalid("SETF index is out of bounds", span));
                            }
                            let stride = dimensions[axis + 1..]
                                .iter()
                                .try_fold(1_usize, |stride, dimension| {
                                    stride.checked_mul(*dimension)
                                })
                                .ok_or_else(|| invalid("SETF index is too large", span))?;
                            offset = offset
                                .checked_add(
                                    index
                                        .checked_mul(stride)
                                        .ok_or_else(|| invalid("SETF index is too large", span))?,
                                )
                                .ok_or_else(|| invalid("SETF index is too large", span))?;
                        }
                        let mut elements =
                            current.array_items().ok_or_else(|| RuntimeError::Type {
                                expected: "ARRAY".to_string(),
                                actual: current.type_name().to_string(),
                                span: Some(span),
                            })?;
                        let slot = elements
                            .get_mut(offset)
                            .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
                        *slot = value.clone();
                        Value::array(dimensions.as_ref().clone(), elements)
                    }
                }
                other => {
                    return Err(RuntimeError::Type {
                        expected: "ARRAY or VECTOR".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    });
                }
            };
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfBitDynamic {
            rank,
            name,
            escaped,
        } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf bit has no value on the stack", span))?
                .primary_value();
            if stack.len() < rank.saturating_add(1) {
                return Err(invalid("setf bit has an incomplete stack", span));
            }
            let indices = stack.split_off(stack.len() - *rank);
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf bit has no target on the stack", span))?
                .primary_value();
            let dimensions = match &current {
                Value::Vector(items) => vec![items.len()],
                Value::Array { dimensions, .. } => dimensions.as_ref().clone(),
                other => {
                    return Err(RuntimeError::Type {
                        expected: "ARRAY".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    });
                }
            };
            if dimensions.len() != *rank {
                return Err(invalid("setf bit has the wrong number of indices", span));
            }
            let indices = indices
                .iter()
                .map(|index| crate::builtins::index_argument("setf bit", index))
                .collect::<Result<Vec<_>, _>>()?;
            let mut offset = 0_usize;
            for (axis, (&dimension, &index)) in dimensions.iter().zip(&indices).enumerate() {
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
            if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
                return Err(RuntimeError::Type {
                    expected: "BIT".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
            let updated = match current {
                Value::Vector(_) => {
                    let mut elements = current.vector_items().unwrap();
                    *elements
                        .get_mut(offset)
                        .ok_or_else(|| invalid("SETF index is out of bounds", span))? =
                        value.clone();
                    Value::vector(elements)
                }
                Value::Array { ref dimensions, .. } => {
                    let mut elements = current.array_items().unwrap();
                    *elements
                        .get_mut(offset)
                        .ok_or_else(|| invalid("SETF index is out of bounds", span))? =
                        value.clone();
                    Value::array(dimensions.as_ref().clone(), elements)
                }
                _ => unreachable!(),
            };
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfElementDynamic {
            operator,
            name,
            escaped,
        } => sequence::execute_element(
            runtime,
            operator,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfSubseqDynamic {
            has_end,
            name,
            escaped,
        } => sequence::execute_subseq(
            runtime,
            *has_end,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushNewList { name, escaped } => list::execute_pushnew(
            runtime,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushNewListOptions {
            name,
            escaped,
            test_not,
            has_key,
            key_before_test,
        } => list::execute_pushnew_options(
            runtime,
            name,
            *escaped,
            *test_not,
            *has_key,
            *key_before_test,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::MapIntoSetfSymbol { name, escaped } => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("map-into has no value on the stack", span))?
                .primary_value();
            if *escaped {
                runtime.set_or_define_exact_in(name, value.clone(), environment, span)?;
            } else {
                runtime.set_or_define_in(name, value.clone(), environment, span)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("map-into has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::Setf(place) | Instruction::MapIntoSetf(place) => {
            let map_into = matches!(instruction, Instruction::MapIntoSetf(_));
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| {
                    invalid(
                        if map_into {
                            "map-into has no value on the stack"
                        } else {
                            "setf has no value on the stack"
                        },
                        span,
                    )
                })?
                .primary_value();
            if map_into {
                runtime.set_map_into_destination(place, value.clone(), environment)?;
            } else {
                runtime.set_place(place, value.clone(), environment)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("setf has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

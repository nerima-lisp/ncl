#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn execute_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
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
        } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf list has no value on the stack", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf list has no target on the stack", span))?
                .primary_value();
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            if elements.is_empty() {
                return Err(invalid("cannot SETF CAR/CDR of NIL", span));
            }
            match operator.as_str() {
                "CAR" | "FIRST" => elements[0] = value.clone(),
                "CDR" | "REST" => {
                    let mut replacement = value.list_items().ok_or_else(|| RuntimeError::Type {
                        expected: "LIST".to_string(),
                        actual: value.type_name().to_string(),
                        span: Some(span),
                    })?;
                    replacement.insert(0, elements[0].clone());
                    elements = replacement;
                }
                _ => return Err(invalid("unsupported native list SETF operator", span)),
            }
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PushList { name, escaped } => {
            let current = stack
                .pop()
                .ok_or_else(|| invalid("push has no target on the stack", span))?
                .primary_value();
            let value = stack
                .pop()
                .ok_or_else(|| invalid("push has no value on the stack", span))?
                .primary_value();
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            elements.insert(0, value);
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated.clone(), environment, span)?;
            }
            stack.push(updated);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfNth {
            index,
            name,
            escaped,
        } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no value on the stack", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no target on the stack", span))?
                .primary_value();
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let slot = elements
                .get_mut(*index)
                .ok_or_else(|| crate::builtins::out_of_bounds("setf nth", *index))?;
            *slot = value.clone();
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfNthDynamic { name, escaped } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no value on the stack", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no target on the stack", span))?
                .primary_value();
            let index = stack
                .pop()
                .ok_or_else(|| invalid("setf nth has no index on the stack", span))?
                .primary_value();
            let index = crate::builtins::index_argument("setf nth", &index)?;
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let slot = elements
                .get_mut(index)
                .ok_or_else(|| crate::builtins::out_of_bounds("setf nth", index))?;
            *slot = value.clone();
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
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
        Instruction::PushNewList { name, escaped } => {
            let current = stack
                .pop()
                .ok_or_else(|| invalid("pushnew has no target on the stack", span))?
                .primary_value();
            let value = stack
                .pop()
                .ok_or_else(|| invalid("pushnew has no value on the stack", span))?
                .primary_value();
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            if elements
                .iter()
                .any(|candidate| crate::builtins::type_predicates::eql_value(&value, candidate))
            {
                stack.push(current);
            } else {
                elements.insert(0, value);
                let updated = Value::list(elements);
                if *escaped {
                    runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
                } else {
                    runtime.set_or_define_in(name, updated.clone(), environment, span)?;
                }
                stack.push(updated);
            }
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PopList { name, escaped } => {
            let current = stack
                .pop()
                .ok_or_else(|| invalid("pop has no target on the stack", span))?
                .primary_value();
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let value = elements.first().cloned().unwrap_or(Value::Nil);
            if !elements.is_empty() {
                elements.remove(0);
            }
            let updated = Value::list(elements);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
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

pub(super) fn execute_parallel_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Psetq(names) => {
            if stack.len() < names.len() {
                return Err(invalid("psetq has fewer values than targets", span));
            }
            let values = stack.split_off(stack.len() - names.len());
            for (name, value) in names.iter().zip(values) {
                let value = value.primary_value();
                runtime.set_or_define_in(name, value, environment, span)?;
            }
            stack.push(Value::Nil);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PsetqExact(names) => {
            if stack.len() < names.len() {
                return Err(invalid("psetq has fewer values than targets", span));
            }
            let values = stack.split_off(stack.len() - names.len());
            for ((name, escaped), value) in names.iter().zip(values) {
                let value = value.primary_value();
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
        Instruction::MultipleValueSetq(names) => {
            let source = pop_value(stack, span, "multiple-value-setq")?;
            let values = source.multiple_values();
            for (index, name) in names.iter().enumerate() {
                let value = values.get(index).cloned().unwrap_or(Value::Nil);
                runtime.set_or_define_in(name, value, environment, span)?;
            }
            stack.push(source.primary_value());
            *program_counter += 1;
            Ok(true)
        }
        Instruction::MultipleValueSetqExact(names) => {
            let source = pop_value(stack, span, "multiple-value-setq")?;
            let values = source.multiple_values();
            for (index, (name, escaped)) in names.iter().enumerate() {
                let value = values.get(index).cloned().unwrap_or(Value::Nil);
                if *escaped {
                    runtime.set_or_define_exact_in(name, value, environment, span)?;
                } else {
                    runtime.set_or_define_in(name, value, environment, span)?;
                }
            }
            stack.push(source.primary_value());
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

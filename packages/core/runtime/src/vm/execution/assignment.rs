#[allow(clippy::wildcard_imports)]
use super::*;

mod list;
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
        } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf element has no value on the stack", span))?
                .primary_value();
            let index = stack
                .pop()
                .ok_or_else(|| invalid("setf element has no index on the stack", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf element has no target on the stack", span))?
                .primary_value();
            let index = crate::builtins::index_argument("setf element", &index)?;
            let updated = match operator.as_str() {
                "ELT" => match current {
                    Value::Nil | Value::List(_) => {
                        let mut elements = current.list_items().unwrap_or_default();
                        *elements
                            .get_mut(index)
                            .ok_or_else(|| invalid("SETF index is out of bounds", span))? =
                            value.clone();
                        Value::list(elements)
                    }
                    Value::Vector(_) => {
                        let mut elements = current.vector_items().unwrap();
                        *elements
                            .get_mut(index)
                            .ok_or_else(|| invalid("SETF index is out of bounds", span))? =
                            value.clone();
                        Value::vector(elements)
                    }
                    Value::String(text) => {
                        let Value::Character(character) = value.clone() else {
                            return Err(RuntimeError::Type {
                                expected: "CHARACTER".to_string(),
                                actual: value.type_name().to_string(),
                                span: Some(span),
                            });
                        };
                        let mut chars = text.chars().collect::<Vec<_>>();
                        *chars
                            .get_mut(index)
                            .ok_or_else(|| invalid("SETF index is out of bounds", span))? =
                            character;
                        Value::string(chars.into_iter().collect::<String>())
                    }
                    other => {
                        return Err(RuntimeError::Type {
                            expected: "SEQUENCE".to_string(),
                            actual: other.type_name().to_string(),
                            span: Some(span),
                        });
                    }
                },
                "CHAR" | "SCHAR" => {
                    let Value::String(text) = current else {
                        return Err(RuntimeError::Type {
                            expected: "STRING".to_string(),
                            actual: current.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    let Value::Character(character) = value.clone() else {
                        return Err(RuntimeError::Type {
                            expected: "CHARACTER".to_string(),
                            actual: value.type_name().to_string(),
                            span: Some(span),
                        });
                    };
                    let mut chars = text.chars().collect::<Vec<_>>();
                    *chars
                        .get_mut(index)
                        .ok_or_else(|| invalid("SETF index is out of bounds", span))? = character;
                    Value::string(chars.into_iter().collect::<String>())
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
        Instruction::SetfSubseqDynamic {
            has_end,
            name,
            escaped,
        } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf subseq has no value on the stack", span))?
                .primary_value();
            let end = if *has_end {
                Some(
                    stack
                        .pop()
                        .ok_or_else(|| invalid("setf subseq has no end on the stack", span))?
                        .primary_value(),
                )
            } else {
                None
            };
            let start = stack
                .pop()
                .ok_or_else(|| invalid("setf subseq has no start on the stack", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf subseq has no target on the stack", span))?
                .primary_value();
            let mut destination = match &current {
                Value::Nil => Vec::new(),
                Value::List(_) | Value::Vector(_) => current
                    .list_items()
                    .unwrap_or_else(|| current.vector_items().unwrap_or_default()),
                Value::String(text) => text.chars().map(Value::Character).collect(),
                other => {
                    return Err(RuntimeError::Type {
                        expected: "SEQUENCE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    });
                }
            };
            let start = crate::builtins::index_argument("setf subseq", &start)?;
            let end = end
                .map(|value| crate::builtins::index_argument("setf subseq", &value))
                .transpose()?
                .unwrap_or(destination.len());
            if start > end || end > destination.len() {
                return Err(invalid("SETF SUBSEQ bounds are invalid", span));
            }
            let replacement = match value.clone() {
                Value::Nil => Vec::new(),
                Value::List(_) | Value::Vector(_) => value
                    .list_items()
                    .unwrap_or_else(|| value.vector_items().unwrap_or_default()),
                Value::String(text) => text.chars().map(Value::Character).collect(),
                other => {
                    return Err(RuntimeError::Type {
                        expected: "SEQUENCE".to_string(),
                        actual: other.type_name().to_string(),
                        span: Some(span),
                    });
                }
            };
            let count = (end - start).min(replacement.len());
            destination[start..start + count].clone_from_slice(&replacement[..count]);
            let updated = match current {
                Value::Nil | Value::List(_) => Value::list(destination),
                Value::Vector(_) => Value::vector(destination),
                Value::String(_) => {
                    let chars = destination
                        .into_iter()
                        .map(|item| match item {
                            Value::Character(character) => Ok(character),
                            other => Err(RuntimeError::Type {
                                expected: "CHARACTER".to_string(),
                                actual: other.type_name().to_string(),
                                span: Some(span),
                            }),
                        })
                        .collect::<Result<String, RuntimeError>>()?;
                    Value::string(chars)
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
        Instruction::SetfGetfDynamic { name, escaped } => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf getf has no value on the stack", span))?
                .primary_value();
            let indicator = stack
                .pop()
                .ok_or_else(|| invalid("setf getf has no indicator on the stack", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("setf getf has no target on the stack", span))?
                .primary_value();
            let mut properties = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            if !properties.len().is_multiple_of(2) {
                return Err(invalid("GETF needs an even property list", span));
            }
            if let Some(index) = (0..properties.len())
                .step_by(2)
                .find(|&index| properties[index].eq_value(&indicator))
                .map(|index| index + 1)
            {
                properties[index] = value.clone();
            } else {
                properties.extend([indicator, value.clone()]);
            }
            let updated = Value::list(properties);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated, environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated, environment, span)?;
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfGetDynamic => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf get has no value on the stack", span))?
                .primary_value();
            let indicator = stack
                .pop()
                .ok_or_else(|| invalid("setf get has no indicator on the stack", span))?
                .primary_value();
            let symbol = stack
                .pop()
                .ok_or_else(|| invalid("setf get has no target on the stack", span))?
                .primary_value();
            if symbol.symbol_reference().is_none() {
                return Err(invalid("setf get target must be a symbol", span));
            }
            let plist = environment.symbol_plist(&symbol).unwrap_or(Value::Nil);
            let mut properties = plist.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: plist.type_name().to_string(),
                span: Some(span),
            })?;
            if !properties.len().is_multiple_of(2) {
                return Err(invalid("SETF GET needs an even property list", span));
            }
            if let Some(index) = (0..properties.len())
                .step_by(2)
                .find(|&index| properties[index].eq_value(&indicator))
                .map(|index| index + 1)
            {
                properties[index] = value.clone();
            } else {
                properties.extend([indicator, value.clone()]);
            }
            environment.set_symbol_plist(&symbol, Value::list(properties));
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfGethashDynamic => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf gethash has no value on the stack", span))?
                .primary_value();
            let table = stack
                .pop()
                .ok_or_else(|| invalid("setf gethash has no table on the stack", span))?
                .primary_value();
            let key = stack
                .pop()
                .ok_or_else(|| invalid("setf gethash has no key on the stack", span))?
                .primary_value();
            let test = table.hash_table_test().ok_or_else(|| RuntimeError::Type {
                expected: "HASH-TABLE".to_string(),
                actual: table.type_name().to_string(),
                span: Some(span),
            })?;
            let entries = table
                .hash_table_entries()
                .ok_or_else(|| RuntimeError::Type {
                    expected: "HASH-TABLE".to_string(),
                    actual: table.type_name().to_string(),
                    span: Some(span),
                })?;
            let mut entries = entries.borrow_mut();
            if let Some((_, slot)) = entries.iter_mut().find(|(stored_key, _)| {
                crate::builtins::hash_table_key_equal(test, stored_key, &key)
            }) {
                *slot = value.clone();
            } else {
                entries.push((key, value.clone()));
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
        Instruction::PushNewListOptions {
            name,
            escaped,
            test_not,
            has_key,
            key_before_test,
        } => {
            let current = stack
                .pop()
                .ok_or_else(|| invalid("pushnew has no target on the stack", span))?
                .primary_value();
            let value = stack
                .pop()
                .ok_or_else(|| invalid("pushnew has no value on the stack", span))?
                .primary_value();
            let (test, key) = if *key_before_test {
                let test = stack
                    .pop()
                    .ok_or_else(|| invalid("pushnew has no test on the stack", span))?
                    .primary_value();
                let key = stack
                    .pop()
                    .ok_or_else(|| invalid("pushnew has no key on the stack", span))?
                    .primary_value();
                (test, Some(key))
            } else {
                let key = if *has_key {
                    Some(
                        stack
                            .pop()
                            .ok_or_else(|| invalid("pushnew has no key on the stack", span))?
                            .primary_value(),
                    )
                } else {
                    None
                };
                let test = stack
                    .pop()
                    .ok_or_else(|| invalid("pushnew has no test on the stack", span))?
                    .primary_value();
                (test, key)
            };
            let test =
                Value::Function(runtime.resolve_function_designator(&test, span, environment)?);
            let key = key
                .filter(|key| key.is_truthy())
                .map(|key| {
                    runtime
                        .resolve_function_designator(&key, span, environment)
                        .map(Value::Function)
                })
                .transpose()?;
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let item_key = key.as_ref().map_or_else(
                || Ok(value.clone()),
                |key| {
                    runtime
                        .apply_in(key, std::slice::from_ref(&value), span, environment)
                        .map(|v| v.primary_value())
                },
            )?;
            let found = elements
                .iter()
                .map(|candidate| {
                    let candidate_key = key.as_ref().map_or_else(
                        || Ok(candidate.clone()),
                        |key| {
                            runtime
                                .apply_in(key, std::slice::from_ref(candidate), span, environment)
                                .map(|v| v.primary_value())
                        },
                    )?;
                    runtime
                        .apply_in(&test, &[item_key.clone(), candidate_key], span, environment)
                        .map(|v| v.primary_value().is_truthy())
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .any(|equal| if *test_not { !equal } else { equal });
            if found {
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

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
        Instruction::SetfFillPointerDynamic { name, escaped } => {
            execute_fill_pointer(runtime, name, *escaped, stack, environment, program_counter, span)
        }
        Instruction::SetfFillPointerValue => {
            execute_fill_pointer_value(stack, program_counter, span)
        }
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
        Instruction::SetfArefValue { rank, operator } => {
            execute_aref_value(*rank, operator, stack, program_counter, span)
        }
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
        Instruction::SetfBitValue { rank } => execute_bit_value(*rank, stack, program_counter, span),
        Instruction::ModifyArefDynamic {
            rank,
            arithmetic,
            operator,
            name,
            escaped,
        } => execute_modify_aref(
            runtime,
            *rank,
            arithmetic,
            operator,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::ArrayMutationDynamic {
            operator,
            rank,
            accessor,
            name,
            escaped,
        } => execute_array_mutation(
            runtime,
            operator,
            *rank,
            accessor,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::ArrayMutationNestedDynamic {
            operator,
            rank,
            accessor,
            accessors,
            name,
            escaped,
        } => execute_nested_array_mutation(
            runtime,
            operator,
            *rank,
            accessor,
            accessors,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::ArrayMutationPushNewOptions {
            rank,
            accessor,
            name,
            escaped,
            test_not,
            has_key,
            key_before_test,
        } => execute_array_pushnew_options(
            runtime,
            *rank,
            accessor,
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
        Instruction::ArrayMutationNestedPushNewOptions {
            rank,
            accessor,
            accessors,
            name,
            escaped,
            test_not,
            has_key,
            key_before_test,
        } => execute_nested_array_pushnew_options(
            runtime,
            *rank,
            accessor,
            accessors,
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
        _ => Ok(false),
    }
}

pub(super) fn execute_fill_pointer(
    runtime: &Runtime, name: &str, escaped: bool, stack: &mut Vec<Value>,
    environment: &Environment, program_counter: &mut usize, span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack.pop().ok_or_else(|| invalid("setf fill-pointer has no value on the stack", span))?.primary_value();
    let current = stack.pop().ok_or_else(|| invalid("setf fill-pointer has no target on the stack", span))?.primary_value();
    set_fill_pointer(&current, &value, span)?;
    if escaped { runtime.set_or_define_exact_in(name, current, environment, span)?; }
    else { runtime.set_or_define_in(name, current, environment, span)?; }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_fill_pointer_value(
    stack: &mut Vec<Value>, program_counter: &mut usize, span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack.pop().ok_or_else(|| invalid("setf fill-pointer has no value on the stack", span))?.primary_value();
    let current = stack.pop().ok_or_else(|| invalid("setf fill-pointer has no target on the stack", span))?.primary_value();
    set_fill_pointer(&current, &value, span)?;
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

fn set_fill_pointer(current: &Value, value: &Value, span: Span) -> Result<(), RuntimeError> {
    let pointer = value.as_integer().and_then(|value| usize::try_from(value).ok()).ok_or_else(|| RuntimeError::Type {
        expected: "NON-NEGATIVE-INTEGER".to_string(), actual: value.type_name().to_string(), span: Some(span),
    })?;
    if current.vector_fill_pointer().flatten().is_none() {
        return Err(RuntimeError::Type { expected: "VECTOR WITH A FILL POINTER".to_string(), actual: current.type_name().to_string(), span: Some(span) });
    }
    let length = current.vector_items().map(|items| items.len()).unwrap_or(0);
    if pointer > length { return Err(invalid("setf fill-pointer exceeds vector length", span)); }
    current.set_vector_fill_pointer(Some(pointer));
    Ok(())
}

fn execute_nested_array_mutation(
    runtime: &Runtime,
    operator: &str,
    rank: usize,
    accessor: &str,
    accessors: &[String],
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < rank + 1 + usize::from(operator == "PUSH") {
        return Err(invalid(
            "nested array mutation has an incomplete stack",
            span,
        ));
    }
    let indices = stack.split_off(stack.len() - rank);
    let base = stack
        .pop()
        .ok_or_else(|| invalid("nested array mutation has no base", span))?
        .primary_value();
    let value = if operator == "PUSH" {
        Some(
            stack
                .pop()
                .ok_or_else(|| invalid("nested array push has no value", span))?
                .primary_value(),
        )
    } else {
        None
    };
    let elements = base.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: base.type_name().to_string(),
        span: Some(span),
    })?;
    let target =
        crate::vm::execution::assignment::list::nested::read(elements.clone(), accessors, span)?;
    let indices = indices
        .iter()
        .map(|index| crate::builtins::index_argument("nested array mutation", index))
        .collect::<Result<Vec<_>, _>>()?;
    let offset = array_mutation_offset(&target, accessor, rank, &indices, span)?;
    let current = array_mutation_item(&target, offset, span)?;
    let mut list = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    let returned = if operator == "POP" {
        if list.is_empty() {
            return Err(invalid("cannot POP from NIL", span));
        }
        list.remove(0)
    } else {
        list.insert(0, value.expect("PUSH value"));
        list[0].clone()
    };
    let updated_target = match &target {
        Value::Vector(_) => {
            target.set_vector_item(offset, Value::list(list));
            target.clone()
        }
        Value::Array { .. } => {
            target.set_array_item(offset, Value::list(list));
            target.clone()
        }
        _ => {
            return Err(RuntimeError::Type {
                expected: "ARRAY or VECTOR".to_string(),
                actual: target.type_name().to_string(),
                span: Some(span),
            });
        }
    };
    let updated_base = Value::list(crate::vm::execution::assignment::list::nested::update(
        elements,
        accessors,
        &updated_target,
        span,
    )?);
    if escaped {
        runtime.set_or_define_exact_in(name, updated_base, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated_base, environment, span)?;
    }
    stack.push(returned);
    *program_counter += 1;
    Ok(true)
}

fn execute_nested_array_pushnew_options(
    runtime: &Runtime,
    rank: usize,
    accessor: &str,
    accessors: &[String],
    name: &str,
    escaped: bool,
    test_not: bool,
    has_key: bool,
    key_before_test: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < rank + 3 + usize::from(has_key) {
        return Err(invalid(
            "nested array pushnew has an incomplete stack",
            span,
        ));
    }
    let indices = stack.split_off(stack.len() - rank);
    let base = stack
        .pop()
        .ok_or_else(|| invalid("nested array pushnew has no base", span))?
        .primary_value();
    let value = stack
        .pop()
        .ok_or_else(|| invalid("nested array pushnew has no value", span))?
        .primary_value();
    let (test, key) = if key_before_test {
        let test = stack
            .pop()
            .ok_or_else(|| invalid("nested array pushnew has no test", span))?
            .primary_value();
        let key = stack
            .pop()
            .ok_or_else(|| invalid("nested array pushnew has no key", span))?
            .primary_value();
        (test, Some(key))
    } else {
        let key = if has_key {
            Some(
                stack
                    .pop()
                    .ok_or_else(|| invalid("nested array pushnew has no key", span))?
                    .primary_value(),
            )
        } else {
            None
        };
        let test = stack
            .pop()
            .ok_or_else(|| invalid("nested array pushnew has no test", span))?
            .primary_value();
        (test, key)
    };
    let test = Value::Function(runtime.resolve_function_designator(&test, span, environment)?);
    let key = key
        .filter(|v| v.is_truthy())
        .map(|v| {
            runtime
                .resolve_function_designator(&v, span, environment)
                .map(Value::Function)
        })
        .transpose()?;
    let base_items = base.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: base.type_name().to_string(),
        span: Some(span),
    })?;
    let target =
        crate::vm::execution::assignment::list::nested::read(base_items.clone(), accessors, span)?;
    let indices = indices
        .iter()
        .map(|v| crate::builtins::index_argument("nested array pushnew", v))
        .collect::<Result<Vec<_>, _>>()?;
    let offset = array_mutation_offset(&target, accessor, rank, &indices, span)?;
    let current = array_mutation_item(&target, offset, span)?;
    let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    let item_key = key.as_ref().map_or_else(
        || Ok(value.clone()),
        |k| {
            runtime
                .apply_in(k, std::slice::from_ref(&value), span, environment)
                .map(|v| v.primary_value())
        },
    )?;
    let found = elements
        .iter()
        .map(|candidate| {
            let candidate_key = key.as_ref().map_or_else(
                || Ok(candidate.clone()),
                |k| {
                    runtime
                        .apply_in(k, std::slice::from_ref(candidate), span, environment)
                        .map(|v| v.primary_value())
                },
            )?;
            runtime
                .apply_in(&test, &[item_key.clone(), candidate_key], span, environment)
                .map(|v| v.primary_value().is_truthy())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|equal| if test_not { !equal } else { equal });
    let result = if found {
        current
    } else {
        elements.insert(0, value);
        Value::list(elements)
    };
    set_array_mutation_item(&target, offset, result.clone(), span)?;
    let updated_base = Value::list(crate::vm::execution::assignment::list::nested::update(
        base_items, accessors, &target, span,
    )?);
    if escaped {
        runtime.set_or_define_exact_in(name, updated_base, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated_base, environment, span)?;
    }
    stack.push(result);
    *program_counter += 1;
    Ok(true)
}

fn execute_array_pushnew_options(
    runtime: &Runtime,
    rank: usize,
    accessor: &str,
    name: &str,
    escaped: bool,
    test_not: bool,
    has_key: bool,
    key_before_test: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < rank + 3 + usize::from(has_key) {
        return Err(invalid("array pushnew has an incomplete stack", span));
    }
    let indices = stack.split_off(stack.len() - rank);
    let target = stack
        .pop()
        .ok_or_else(|| invalid("array pushnew has no target", span))?
        .primary_value();
    let value = stack
        .pop()
        .ok_or_else(|| invalid("array pushnew has no value", span))?
        .primary_value();
    let (test, key) = if key_before_test {
        let test = stack
            .pop()
            .ok_or_else(|| invalid("array pushnew has no test", span))?
            .primary_value();
        let key = stack
            .pop()
            .ok_or_else(|| invalid("array pushnew has no key", span))?
            .primary_value();
        (test, Some(key))
    } else {
        let key = if has_key {
            Some(
                stack
                    .pop()
                    .ok_or_else(|| invalid("array pushnew has no key", span))?
                    .primary_value(),
            )
        } else {
            None
        };
        let test = stack
            .pop()
            .ok_or_else(|| invalid("array pushnew has no test", span))?
            .primary_value();
        (test, key)
    };
    let test = Value::Function(runtime.resolve_function_designator(&test, span, environment)?);
    let key = key
        .filter(|key| key.is_truthy())
        .map(|key| {
            runtime
                .resolve_function_designator(&key, span, environment)
                .map(Value::Function)
        })
        .transpose()?;
    let indices = indices
        .iter()
        .map(|index| crate::builtins::index_argument("array pushnew", index))
        .collect::<Result<Vec<_>, _>>()?;
    let offset = array_mutation_offset(&target, accessor, rank, &indices, span)?;
    let current = array_mutation_item(&target, offset, span)?;
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
        .any(|equal| if test_not { !equal } else { equal });
    let result = if found {
        current
    } else {
        elements.insert(0, value);
        let updated = Value::list(elements);
        set_array_mutation_item(&target, offset, updated.clone(), span)?;
        updated
    };
    store_array_value(
        runtime,
        name,
        escaped,
        target,
        result,
        stack,
        environment,
        program_counter,
        span,
    )
}

fn execute_array_mutation(
    runtime: &Runtime,
    operator: &str,
    rank: usize,
    accessor: &str,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < rank + 1 {
        return Err(invalid("array mutation has an incomplete stack", span));
    }
    let indices = stack.split_off(stack.len() - rank);
    let target = stack
        .pop()
        .ok_or_else(|| invalid("array mutation has no target", span))?
        .primary_value();
    let value = if operator == "PUSH" {
        Some(
            stack
                .pop()
                .ok_or_else(|| invalid("push array has no value", span))?
                .primary_value(),
        )
    } else {
        None
    };
    let indices = indices
        .iter()
        .map(|index| crate::builtins::index_argument("array mutation", index))
        .collect::<Result<Vec<_>, _>>()?;
    let offset = match &target {
        Value::Vector(_) if rank == 1 => indices[0],
        Value::Array { dimensions, .. } if accessor != "SVREF" => {
            if accessor == "ROW-MAJOR-AREF" && rank == 1 {
                indices[0]
            } else {
                array_offset(
                    dimensions,
                    rank,
                    &indices,
                    "array mutation has the wrong number of indices",
                    span,
                )?
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
    let current = match &target {
        Value::Vector(_) => target
            .vector_items()
            .and_then(|items| items.get(offset).cloned()),
        Value::Array { .. } => target
            .array_items()
            .and_then(|items| items.get(offset).cloned()),
        _ => None,
    }
    .ok_or_else(|| invalid("array mutation index is out of bounds", span))?;
    let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    let result = if operator == "POP" {
        if elements.is_empty() {
            return Err(invalid("cannot POP from NIL", span));
        }
        elements.remove(0)
    } else {
        elements.insert(0, value.expect("PUSH value"));
        elements[0].clone()
    };
    let updated = Value::list(elements);
    match &target {
        Value::Vector(_) => {
            target.set_vector_item(offset, updated);
        }
        Value::Array { .. } => {
            target.set_array_item(offset, updated);
        }
        _ => unreachable!(),
    }
    store_array_value(
        runtime,
        name,
        escaped,
        target,
        result,
        stack,
        environment,
        program_counter,
        span,
    )
}

fn array_mutation_offset(
    target: &Value,
    accessor: &str,
    rank: usize,
    indices: &[usize],
    span: Span,
) -> Result<usize, RuntimeError> {
    match target {
        Value::Vector(_) if rank == 1 => Ok(indices[0]),
        Value::Array { dimensions, .. } if accessor != "SVREF" => {
            if accessor == "ROW-MAJOR-AREF" && rank == 1 {
                Ok(indices[0])
            } else {
                array_offset(
                    dimensions,
                    rank,
                    indices,
                    "array pushnew has the wrong number of indices",
                    span,
                )
            }
        }
        other => Err(RuntimeError::Type {
            expected: "ARRAY or VECTOR".to_string(),
            actual: other.type_name().to_string(),
            span: Some(span),
        }),
    }
}

fn array_mutation_item(target: &Value, offset: usize, span: Span) -> Result<Value, RuntimeError> {
    match target {
        Value::Vector(_) => target
            .vector_items()
            .and_then(|items| items.get(offset).cloned()),
        Value::Array { .. } => target
            .array_items()
            .and_then(|items| items.get(offset).cloned()),
        _ => None,
    }
    .ok_or_else(|| invalid("array pushnew index is out of bounds", span))
}

fn set_array_mutation_item(
    target: &Value,
    offset: usize,
    value: Value,
    span: Span,
) -> Result<(), RuntimeError> {
    let updated = match target {
        Value::Vector(_) => target.set_vector_item(offset, value),
        Value::Array { .. } => target.set_array_item(offset, value),
        _ => None,
    };
    updated
        .ok_or_else(|| invalid("array pushnew index is out of bounds", span))
        .map(|_| ())
}

fn execute_modify_aref(
    runtime: &Runtime,
    rank: usize,
    arithmetic: &str,
    operator: &str,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let delta = stack
        .pop()
        .ok_or_else(|| invalid("modify aref has no delta", span))?
        .primary_value();
    if stack.len() < rank + 1 {
        return Err(invalid("modify aref has an incomplete stack", span));
    }
    let indices = stack.split_off(stack.len() - rank);
    let indices = indices
        .iter()
        .map(|index| crate::builtins::index_argument("modify array accessor", index))
        .collect::<Result<Vec<_>, _>>()?;
    let target = stack
        .pop()
        .ok_or_else(|| invalid("modify aref has no target", span))?
        .primary_value();
    let offset = match &target {
        Value::Vector(_) if rank == 1 => indices[0],
        Value::Array { dimensions, .. } if operator != "SVREF" => {
            if operator == "ROW-MAJOR-AREF" && rank == 1 {
                indices[0]
            } else {
                array_offset(
                    dimensions,
                    rank,
                    &indices,
                    "modify array has the wrong number of indices",
                    span,
                )?
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
    let current = match &target {
        Value::Vector(_) => target
            .vector_items()
            .and_then(|items| items.get(offset).cloned()),
        Value::Array { .. } => target
            .array_items()
            .and_then(|items| items.get(offset).cloned()),
        _ => None,
    }
    .ok_or_else(|| invalid("MODIFY array index is out of bounds", span))?;
    let value = runtime
        .apply_in(
            &Value::symbol(arithmetic.to_string()),
            &[current, delta],
            span,
            environment,
        )?
        .primary_value();
    match &target {
        Value::Vector(_) => {
            target.set_vector_item(offset, value.clone());
        }
        Value::Array { .. } => {
            target.set_array_item(offset, value.clone());
        }
        _ => unreachable!(),
    }
    store_array_value(
        runtime,
        name,
        escaped,
        target,
        value,
        stack,
        environment,
        program_counter,
        span,
    )
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
        Value::Vector(_) | Value::MutableString(_) => {
            if rank != 1 {
                return Err(invalid("setf aref requires one vector index", span));
            }
            current
                .set_vector_item(indices[0], value.clone())
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
            current
                .set_array_item(offset, value.clone())
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

fn execute_aref_value(
    rank: usize,
    operator: &str,
    stack: &mut Vec<Value>,
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
    match &current {
        Value::Vector(_) | Value::MutableString(_) => {
            if rank != 1 {
                return Err(invalid("setf aref requires one vector index", span));
            }
            current
                .set_vector_item(indices[0], value.clone())
                .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
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
            current
                .set_array_item(offset, value.clone())
                .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
        }
        other => {
            return Err(RuntimeError::Type {
                expected: "ARRAY or VECTOR".to_string(),
                actual: other.type_name().to_string(),
                span: Some(span),
            });
        }
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
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
            current
                .set_vector_item(offset, value.clone())
                .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
            current.clone()
        }
        Value::Array { .. } => {
            current
                .set_array_item(offset, value.clone())
                .ok_or_else(|| invalid("SETF index is out of bounds", span))?;
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

fn execute_bit_value(
    rank: usize,
    stack: &mut Vec<Value>,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack.pop().ok_or_else(|| invalid("setf bit has no value on the stack", span))?.primary_value();
    if stack.len() < rank.saturating_add(1) { return Err(invalid("setf bit has an incomplete stack", span)); }
    let indices = stack.split_off(stack.len() - rank);
    let current = stack.pop().ok_or_else(|| invalid("setf bit has no target on the stack", span))?.primary_value();
    let dimensions = match &current {
        Value::Vector(items) => vec![items.borrow().len()],
        Value::Array { dimensions, .. } => dimensions.as_ref().clone(),
        other => return Err(RuntimeError::Type { expected: "ARRAY".to_string(), actual: other.type_name().to_string(), span: Some(span) }),
    };
    let indices = indices.iter().map(|index| crate::builtins::index_argument("setf bit", index)).collect::<Result<Vec<_>, _>>()?;
    let offset = array_offset(&dimensions, rank, &indices, "setf bit has the wrong number of indices", span)?;
    if !matches!(&value, Value::Integer(bit) if *bit == 0 || *bit == 1) {
        return Err(RuntimeError::Type { expected: "BIT".to_string(), actual: value.type_name().to_string(), span: Some(span) });
    }
    match &current {
        Value::Vector(_) => { current.set_vector_item(offset, value.clone()).ok_or_else(|| invalid("SETF index is out of bounds", span))?; }
        Value::Array { .. } => { current.set_array_item(offset, value.clone()).ok_or_else(|| invalid("SETF index is out of bounds", span))?; }
        _ => unreachable!(),
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
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

#[allow(clippy::wildcard_imports)]
use super::super::*;

mod getf;
mod gethash;
mod slot_value;

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if getf::execute(
        runtime,
        instruction,
        stack,
        environment,
        program_counter,
        span,
    )? {
        return Ok(true);
    }
    if gethash::execute(
        runtime,
        instruction,
        stack,
        environment,
        program_counter,
        span,
    )? {
        return Ok(true);
    }
    if slot_value::execute(
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
        Instruction::Remf { name, escaped } => {
            let indicator = stack
                .pop()
                .ok_or_else(|| invalid("remf has no indicator", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("remf has no property list", span))?
                .primary_value();
            let mut properties = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            if !properties.len().is_multiple_of(2) {
                return Err(invalid("REMF needs an even property list", span));
            }
            let found_index = (0..properties.len())
                .step_by(2)
                .find(|&index| crate::builtins::eql_value(&properties[index], &indicator));
            let found = found_index.is_some();
            if let Some(index) = found_index {
                properties.drain(index..=index + 1);
            }
            let updated = Value::list(properties);
            if *escaped {
                runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated.clone(), environment, span)?;
            }
            stack.push(Value::values(vec![updated, Value::boolean(found)]));
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
        Instruction::PushNewGethash => {
            let table = stack
                .pop()
                .ok_or_else(|| invalid("pushnew gethash has no table", span))?
                .primary_value();
            let key = stack
                .pop()
                .ok_or_else(|| invalid("pushnew gethash has no key", span))?
                .primary_value();
            let value = stack
                .pop()
                .ok_or_else(|| invalid("pushnew gethash has no value", span))?
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
                let mut values = slot.list_items().ok_or_else(|| RuntimeError::Type {
                    expected: "LIST".to_string(),
                    actual: slot.type_name().to_string(),
                    span: Some(span),
                })?;
                if !values
                    .iter()
                    .any(|candidate| crate::builtins::type_predicates::eql_value(&value, candidate))
                {
                    values.insert(0, value);
                    *slot = Value::list(values);
                }
                stack.push(slot.clone());
            } else {
                let updated = Value::list(vec![value]);
                entries.push((key, updated.clone()));
                stack.push(updated);
            }
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PushNewGethashOptions {
            test_not,
            has_key,
            key_before_test,
        } => {
            let table = stack
                .pop()
                .ok_or_else(|| invalid("pushnew gethash has no table", span))?
                .primary_value();
            let key = stack
                .pop()
                .ok_or_else(|| invalid("pushnew gethash has no key", span))?
                .primary_value();
            let value = stack
                .pop()
                .ok_or_else(|| invalid("pushnew gethash has no value", span))?
                .primary_value();
            let (test, item_key) = if *key_before_test {
                let test = stack
                    .pop()
                    .ok_or_else(|| invalid("pushnew gethash has no test", span))?
                    .primary_value();
                let key_fn = stack
                    .pop()
                    .ok_or_else(|| invalid("pushnew gethash has no key function", span))?
                    .primary_value();
                (test, Some(key_fn))
            } else {
                let key_fn = if *has_key {
                    Some(
                        stack
                            .pop()
                            .ok_or_else(|| invalid("pushnew gethash has no key function", span))?
                            .primary_value(),
                    )
                } else {
                    None
                };
                let test = stack
                    .pop()
                    .ok_or_else(|| invalid("pushnew gethash has no test", span))?
                    .primary_value();
                (test, key_fn)
            };
            let test =
                Value::Function(runtime.resolve_function_designator(&test, span, environment)?);
            let key_fn = item_key
                .map(|v| {
                    runtime
                        .resolve_function_designator(&v, span, environment)
                        .map(Value::Function)
                })
                .transpose()?;
            let test_key = key_fn.as_ref().map_or_else(
                || Ok(value.clone()),
                |f| {
                    runtime
                        .apply_in(f, std::slice::from_ref(&value), span, environment)
                        .map(|v| v.primary_value())
                },
            )?;
            let hash_test = table.hash_table_test().ok_or_else(|| RuntimeError::Type {
                expected: "HASH-TABLE".into(),
                actual: table.type_name().into(),
                span: Some(span),
            })?;
            let entries = table
                .hash_table_entries()
                .ok_or_else(|| RuntimeError::Type {
                    expected: "HASH-TABLE".into(),
                    actual: table.type_name().into(),
                    span: Some(span),
                })?;
            let mut entries = entries.borrow_mut();
            if let Some((_, slot)) = entries.iter_mut().find(|(stored_key, _)| {
                crate::builtins::hash_table_key_equal(hash_test, stored_key, &key)
            }) {
                let mut values = slot.list_items().ok_or_else(|| RuntimeError::Type {
                    expected: "LIST".into(),
                    actual: slot.type_name().into(),
                    span: Some(span),
                })?;
                let found = values
                    .iter()
                    .map(|candidate| {
                        let candidate_key = key_fn.as_ref().map_or_else(
                            || Ok(candidate.clone()),
                            |f| {
                                runtime
                                    .apply_in(f, std::slice::from_ref(candidate), span, environment)
                                    .map(|v| v.primary_value())
                            },
                        )?;
                        runtime
                            .apply_in(&test, &[test_key.clone(), candidate_key], span, environment)
                            .map(|v| {
                                let equal = v.primary_value().is_truthy();
                                if *test_not {
                                    !equal
                                } else {
                                    equal
                                }
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .any(|v| v);
                if !found {
                    values.insert(0, value);
                    *slot = Value::list(values);
                }
                stack.push(slot.clone());
            } else {
                let updated = Value::list(vec![value]);
                entries.push((key, updated.clone()));
                stack.push(updated);
            }
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PopGethash => {
            let table = stack
                .pop()
                .ok_or_else(|| invalid("pop gethash has no table", span))?
                .primary_value();
            let key = stack
                .pop()
                .ok_or_else(|| invalid("pop gethash has no key", span))?
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
            let popped = if let Some((_, slot)) = entries.iter_mut().find(|(stored_key, _)| {
                crate::builtins::hash_table_key_equal(test, stored_key, &key)
            }) {
                let mut values = slot.list_items().ok_or_else(|| RuntimeError::Type {
                    expected: "LIST".to_string(),
                    actual: slot.type_name().to_string(),
                    span: Some(span),
                })?;
                let popped = values.first().cloned().unwrap_or(Value::Nil);
                if !values.is_empty() {
                    values.remove(0);
                }
                *slot = Value::list(values);
                popped
            } else {
                Value::Nil
            };
            stack.push(popped);
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

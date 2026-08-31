#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
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
    match operator {
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
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_place_mutation(
    runtime: &Runtime,
    operator: &str,
    accessor: &str,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let target = stack
        .pop()
        .ok_or_else(|| invalid("list place has no target", span))?
        .primary_value();
    let mut outer = target.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: target.type_name().to_string(),
        span: Some(span),
    })?;
    if outer.is_empty() {
        return Err(invalid("cannot mutate a list place of NIL", span));
    }
    let current = match accessor {
        "CAR" | "FIRST" => outer[0].clone(),
        "CDR" | "REST" => Value::list(outer[1..].to_vec()),
        _ => return Err(invalid("unsupported native list accessor", span)),
    };
    let (result, updated_place) = match operator {
        "PUSH" => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("push has no value", span))?
                .primary_value();
            let mut items = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            items.insert(0, value);
            let updated = Value::list(items);
            (updated.clone(), updated)
        }
        "POP" => {
            let mut items = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(span),
            })?;
            let value = items.first().cloned().unwrap_or(Value::Nil);
            if !items.is_empty() {
                items.remove(0);
            }
            (value, Value::list(items))
        }
        _ => return Err(invalid("unsupported native list place mutation", span)),
    };
    match accessor {
        "CAR" | "FIRST" => outer[0] = updated_place,
        "CDR" | "REST" => {
            outer = vec![outer[0].clone()];
            outer.extend(updated_place.list_items().unwrap_or_default());
        }
        _ => unreachable!(),
    }
    let updated = Value::list(outer);
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(result);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_pushnew(
    runtime: &Runtime,
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
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
        if escaped {
            runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
        } else {
            runtime.set_or_define_in(name, updated.clone(), environment, span)?;
        }
        stack.push(updated);
    }
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_pushnew_options(
    runtime: &Runtime,
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
    let current = stack
        .pop()
        .ok_or_else(|| invalid("pushnew has no target on the stack", span))?
        .primary_value();
    let value = stack
        .pop()
        .ok_or_else(|| invalid("pushnew has no value on the stack", span))?
        .primary_value();
    let (test, key) = if key_before_test {
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
        let key = if has_key {
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
    let test = Value::Function(runtime.resolve_function_designator(&test, span, environment)?);
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
        .any(|equal| if test_not { !equal } else { equal });
    if found {
        stack.push(current);
    } else {
        elements.insert(0, value);
        let updated = Value::list(elements);
        if escaped {
            runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
        } else {
            runtime.set_or_define_in(name, updated.clone(), environment, span)?;
        }
        stack.push(updated);
    }
    *program_counter += 1;
    Ok(true)
}

#[allow(clippy::wildcard_imports)]
use super::super::*;

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

pub(super) fn execute_place_pushnew_options(
    runtime: &Runtime,
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
    let _inner = stack
        .pop()
        .ok_or_else(|| invalid("list place has no target", span))?
        .primary_value();
    let outer = if escaped {
        runtime.lookup_exact_in(name, environment)
    } else {
        runtime.lookup_in(name, environment)
    }
    .ok_or_else(|| invalid("list place has no outer target", span))?
    .primary_value();
    let mut outer_items = outer.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: outer.type_name().to_string(),
        span: Some(span),
    })?;
    if outer_items.is_empty() {
        return Err(invalid("cannot mutate a list place of NIL", span));
    }
    let current = match accessor {
        "CAR" | "FIRST" => outer_items[0].clone(),
        "CDR" | "REST" => Value::list(outer_items[1..].to_vec()),
        _ => return Err(invalid("unsupported native list accessor", span)),
    };
    let value = stack
        .last()
        .cloned()
        .ok_or_else(|| invalid("pushnew has no value", span))?;
    stack.pop();
    stack.push(value);
    stack.push(current);
    execute_pushnew_options(
        runtime,
        name,
        escaped,
        test_not,
        has_key,
        key_before_test,
        stack,
        environment,
        program_counter,
        span,
    )?;
    let updated_place = stack
        .pop()
        .ok_or_else(|| invalid("pushnew produced no result", span))?;
    match accessor {
        "CAR" | "FIRST" => outer_items[0] = updated_place.clone(),
        "CDR" | "REST" => {
            outer_items = vec![outer_items[0].clone()];
            outer_items.extend(updated_place.list_items().unwrap_or_default());
        }
        _ => unreachable!(),
    }
    let updated_outer = Value::list(outer_items);
    if escaped {
        runtime.set_or_define_exact_in(name, updated_outer, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated_outer, environment, span)?;
    }
    stack.push(updated_place);
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

#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute_nested_place_pushnew_options(
    runtime: &Runtime,
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
    let target = stack
        .pop()
        .ok_or_else(|| invalid("list place has no target", span))?
        .primary_value();
    let value = stack
        .pop()
        .ok_or_else(|| invalid("pushnew has no value", span))?
        .primary_value();
    let (test, key) =
        pop_comparison_functions(runtime, has_key, key_before_test, stack, environment, span)?;
    let elements = target.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: target.type_name().to_string(),
        span: Some(span),
    })?;
    let (updated, result) = mutate_nested_pushnew(
        runtime,
        elements,
        accessors,
        &value,
        &test,
        key.as_ref(),
        test_not,
        environment,
        span,
    )?;
    let updated = Value::list(updated);
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(result);
    *program_counter += 1;
    Ok(true)
}

fn pop_comparison_functions(
    runtime: &Runtime,
    has_key: bool,
    key_before_test: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(Value, Option<Value>), RuntimeError> {
    let (test, key) = if key_before_test {
        let test = stack
            .pop()
            .ok_or_else(|| invalid("pushnew has no test", span))?;
        let key = stack
            .pop()
            .ok_or_else(|| invalid("pushnew has no key", span))?;
        (test, Some(key))
    } else {
        let key = if has_key {
            Some(
                stack
                    .pop()
                    .ok_or_else(|| invalid("pushnew has no key", span))?,
            )
        } else {
            None
        };
        let test = stack
            .pop()
            .ok_or_else(|| invalid("pushnew has no test", span))?;
        (test, key)
    };
    let test = Value::Function(runtime.resolve_function_designator(
        &test.primary_value(),
        span,
        environment,
    )?);
    let key = key
        .filter(|key| key.primary_value().is_truthy())
        .map(|key| {
            runtime
                .resolve_function_designator(&key.primary_value(), span, environment)
                .map(Value::Function)
        })
        .transpose()?;
    Ok((test, key))
}

fn mutate_nested_pushnew(
    runtime: &Runtime,
    mut elements: Vec<Value>,
    accessors: &[String],
    value: &Value,
    test: &Value,
    key: Option<&Value>,
    test_not: bool,
    environment: &Environment,
    span: Span,
) -> Result<(Vec<Value>, Value), RuntimeError> {
    if elements.is_empty() {
        return Err(invalid("cannot mutate a list place of NIL", span));
    }
    if accessors.len() > 1 {
        let child = match accessors[0].as_str() {
            "CAR" | "FIRST" => elements[0].list_items(),
            "CDR" | "REST" => Some(elements[1..].to_vec()),
            accessor if super::list::fixed_accessor_index(accessor).is_some() => elements
                .get(super::list::fixed_accessor_index(accessor).expect("checked fixed accessor"))
                .and_then(Value::list_items),
            _ => None,
        }
        .ok_or_else(|| invalid("unsupported native nested list accessor", span))?;
        let (child, result) = mutate_nested_pushnew(
            runtime,
            child,
            &accessors[1..],
            value,
            test,
            key,
            test_not,
            environment,
            span,
        )?;
        match accessors[0].as_str() {
            "CAR" | "FIRST" => elements[0] = Value::list(child),
            "CDR" | "REST" => {
                elements.truncate(1);
                elements.extend(child);
            }
            accessor if super::list::fixed_accessor_index(accessor).is_some() => {
                let index =
                    super::list::fixed_accessor_index(accessor).expect("checked fixed accessor");
                elements[index] = Value::list(child);
            }
            _ => unreachable!(),
        }
        return Ok((elements, result));
    }
    let current = match accessors[0].as_str() {
        "CAR" | "FIRST" => elements[0].clone(),
        "CDR" | "REST" => Value::list(elements[1..].to_vec()),
        accessor if super::list::fixed_accessor_index(accessor).is_some() => elements
            .get(super::list::fixed_accessor_index(accessor).expect("checked fixed accessor"))
            .cloned()
            .ok_or_else(|| invalid("list accessor index is out of bounds", span))?,
        _ => return Err(invalid("unsupported native nested list accessor", span)),
    };
    let mut items = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    let item_key = apply_key(runtime, key, value, environment, span)?;
    let found = items
        .iter()
        .map(|candidate| {
            let candidate_key = apply_key(runtime, key, candidate, environment, span)?;
            runtime
                .apply_in(test, &[item_key.clone(), candidate_key], span, environment)
                .map(|v| v.primary_value().is_truthy())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|equal| if test_not { !equal } else { equal });
    let result = if found {
        current
    } else {
        items.insert(0, value.clone());
        Value::list(items.clone())
    };
    let updated = Value::list(items);
    match accessors[0].as_str() {
        "CAR" | "FIRST" => elements[0] = updated,
        "CDR" | "REST" => {
            elements.truncate(1);
            elements.extend(updated.list_items().unwrap_or_default());
        }
        accessor if super::list::fixed_accessor_index(accessor).is_some() => {
            let index =
                super::list::fixed_accessor_index(accessor).expect("checked fixed accessor");
            elements[index] = updated;
        }
        _ => unreachable!(),
    }
    Ok((elements, result))
}

fn apply_key(
    runtime: &Runtime,
    key: Option<&Value>,
    value: &Value,
    environment: &Environment,
    span: Span,
) -> Result<Value, RuntimeError> {
    key.map_or_else(
        || Ok(value.clone()),
        |key| {
            runtime
                .apply_in(key, std::slice::from_ref(value), span, environment)
                .map(|v| v.primary_value())
        },
    )
}

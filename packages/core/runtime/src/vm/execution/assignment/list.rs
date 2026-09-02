#[allow(clippy::wildcard_imports)]
use super::super::*;

mod accessor;
#[path = "list_mutation.rs"]
pub(super) mod mutation;
pub(crate) mod nested;
pub(super) use accessor::fixed_index as fixed_accessor_index;

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
        "CAR" | "FIRST" => {}
        "CDR" | "REST" => {
            let mut replacement = value.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?;
            replacement.insert(0, elements[0].clone());
        }
        accessor if fixed_accessor_index(accessor).is_some() => {
            let index = fixed_accessor_index(accessor).expect("checked fixed accessor");
            let slot = elements
                .get_mut(index)
                .ok_or_else(|| invalid("list accessor index is out of bounds", span))?;
            *slot = value.clone();
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

pub(super) fn execute_value(
    operator: &str,
    stack: &mut Vec<Value>,
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
    let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    if elements.is_empty() {
        return Err(invalid("cannot SETF CAR/CDR of NIL", span));
    }
    match operator {
        "CAR" | "FIRST" => {}
        "CDR" | "REST" => {
            let mut replacement = value.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: value.type_name().to_string(),
                span: Some(span),
            })?;
            replacement.insert(0, elements[0].clone());
        }
        accessor if fixed_accessor_index(accessor).is_some() => {
            let index = fixed_accessor_index(accessor).expect("checked fixed accessor");
            if elements.get(index).is_none() {
                return Err(invalid("list accessor index is out of bounds", span));
            }
        }
        _ => return Err(invalid("unsupported native list SETF operator", span)),
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_nested(
    runtime: &Runtime,
    accessors: &[String],
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
    let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    let updated = Value::list(nested::update(elements, accessors, &value, span)?);
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_nested_nth_dynamic(
    runtime: &Runtime,
    accessors: &[String],
    name: &str,
    escaped: bool,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
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
    let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    let updated = Value::list(nested::update_dynamic(
        elements, accessors, index, &value, span,
    )?);
    if escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(crate) fn execute_parallel(
    runtime: &Runtime,
    places: &[(Vec<String>, String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() {
        return Err(invalid("psetf has fewer values than list places", span));
    }
    let values = stack.split_off(stack.len() - places.len());
    let mut last = Value::Nil;
    for ((accessors, name, escaped), value) in places.iter().zip(values) {
        last = value.primary_value();
        let current = if *escaped {
            runtime.lookup_exact_in(name, environment)
        } else {
            runtime.lookup_in(name, environment)
        }
        .ok_or_else(|| invalid("unbound PSETF list target", span))?;
        let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: current.type_name().to_string(),
            span: Some(span),
        })?;
        let updated = Value::list(nested::update(elements, accessors, &last, span)?);
        if *escaped {
            runtime.set_or_define_exact_in(name, updated, environment, span)?;
        } else {
            runtime.set_or_define_in(name, updated, environment, span)?;
        }
    }
    stack.push(last);
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
        accessor if fixed_accessor_index(accessor).is_some() => outer
            .get(fixed_accessor_index(accessor).expect("checked fixed accessor"))
            .cloned()
            .ok_or_else(|| invalid("list accessor index is out of bounds", span))?,
        _ => return Err(invalid("unsupported native list accessor", span)),
    };
    let (result, updated_place) = mutation::apply(operator, current, stack, span)?;

    match accessor {
        "CAR" | "FIRST" => outer[0] = updated_place,
        "CDR" | "REST" => {
            outer = vec![outer[0].clone()];
            outer.extend(updated_place.list_items().unwrap_or_default());
        }
        accessor if fixed_accessor_index(accessor).is_some() => {
            let index = fixed_accessor_index(accessor).expect("checked fixed accessor");
            let slot = outer
                .get_mut(index)
                .ok_or_else(|| invalid("list accessor index is out of bounds", span))?;
            *slot = updated_place;
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

pub(super) fn execute_nested_place_mutation(
    runtime: &Runtime,
    accessors: &[String],
    operator: &str,
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
    let value = if matches!(operator, "PUSH" | "PUSHNEW") {
        Some(
            stack
                .pop()
                .ok_or_else(|| invalid("push has no value", span))?
                .primary_value(),
        )
    } else {
        None
    };
    let elements = target.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".into(),
        actual: target.type_name().into(),
        span: Some(span),
    })?;
    let (updated, result) = mutate_nested(elements, accessors, operator, value, span)?;
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

fn mutate_nested(
    mut elements: Vec<Value>,
    accessors: &[String],
    operator: &str,
    value: Option<Value>,
    span: Span,
) -> Result<(Vec<Value>, Value), RuntimeError> {
    if elements.is_empty() {
        return Err(invalid("cannot mutate a list place of NIL", span));
    }
    if accessors.len() > 1 {
        let child = match accessors[0].as_str() {
            "CAR" | "FIRST" => elements[0].list_items(),
            "CDR" | "REST" => Some(elements[1..].to_vec()),
            accessor if fixed_accessor_index(accessor).is_some() => elements
                .get(fixed_accessor_index(accessor).expect("checked fixed accessor"))
                .and_then(Value::list_items),
            _ => None,
        }
        .ok_or_else(|| invalid("unsupported native nested list accessor", span))?;
        let (child, result) = mutate_nested(child, &accessors[1..], operator, value, span)?;
        match accessors[0].as_str() {
            "CAR" | "FIRST" => elements[0] = Value::list(child),
            "CDR" | "REST" => {
                elements.truncate(1);
                elements.extend(child);
            }
            accessor if fixed_accessor_index(accessor).is_some() => {
                let index = fixed_accessor_index(accessor).expect("checked fixed accessor");
                elements[index] = Value::list(child);
            }
            _ => unreachable!(),
        }
        return Ok((elements, result));
    }
    let current = match accessors[0].as_str() {
        "CAR" | "FIRST" => elements[0].clone(),
        "CDR" | "REST" => Value::list(elements[1..].to_vec()),
        accessor if fixed_accessor_index(accessor).is_some() => elements
            .get(fixed_accessor_index(accessor).expect("checked fixed accessor"))
            .cloned()
            .ok_or_else(|| invalid("list accessor index is out of bounds", span))?,
        _ => return Err(invalid("unsupported native nested list accessor", span)),
    };
    let mut items = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".into(),
        actual: current.type_name().into(),
        span: Some(span),
    })?;
    let result = match operator {
        "PUSH" => {
            let value = value.expect("PUSH value");
            items.insert(0, value);
            Value::list(items.clone())
        }
        "PUSHNEW" => {
            let value = value.expect("PUSHNEW value");
            if items
                .iter()
                .any(|candidate| crate::builtins::type_predicates::eql_value(&value, candidate))
            {
                current
            } else {
                items.insert(0, value);
                Value::list(items.clone())
            }
        }
        "POP" => {
            let value = items.first().cloned().unwrap_or(Value::Nil);
            if !items.is_empty() {
                items.remove(0);
            }
            value
        }
        _ => {
            return Err(invalid(
                "unsupported native nested list place mutation",
                span,
            ));
        }
    };
    let updated = Value::list(items);
    match accessors[0].as_str() {
        "CAR" | "FIRST" => elements[0] = updated,
        "CDR" | "REST" => {
            elements.truncate(1);
            elements.extend(updated.list_items().unwrap_or_default());
        }
        accessor if fixed_accessor_index(accessor).is_some() => {
            let index = fixed_accessor_index(accessor).expect("checked fixed accessor");
            elements[index] = updated;
        }
        _ => unreachable!(),
    }
    Ok((elements, result))
}

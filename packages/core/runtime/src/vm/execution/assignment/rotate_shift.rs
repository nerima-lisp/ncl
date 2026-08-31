#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute_rotatef(
    runtime: &Runtime,
    places: &[(String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() {
        return Err(invalid("rotatef has too few values on the stack", span));
    }
    let values = stack.split_off(stack.len() - places.len());
    for (index, (name, escaped)) in places.iter().enumerate() {
        let value = values[(index + values.len() - 1) % values.len()]
            .clone()
            .primary_value();
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

pub(super) fn execute_shiftf(
    runtime: &Runtime,
    places: &[(String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() + 1 {
        return Err(invalid("shiftf has too few values on the stack", span));
    }
    let values = stack.split_off(stack.len() - places.len() - 1);
    let old_first = values[0].clone().primary_value();
    for (index, (name, escaped)) in places.iter().enumerate() {
        let value = values
            .get(index + 1)
            .cloned()
            .unwrap_or_else(|| Value::Nil)
            .primary_value();
        if *escaped {
            runtime.set_or_define_exact_in(name, value, environment, span)?;
        } else {
            runtime.set_or_define_in(name, value, environment, span)?;
        }
    }
    stack.push(old_first);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_rotatef_nested(
    places: &[(Vec<String>, String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    runtime: &Runtime,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() {
        return Err(invalid("rotatef has too few values on the stack", span));
    }
    let roots = stack.split_off(stack.len() - places.len());
    let mut values = Vec::with_capacity(places.len());
    for ((accessors, _, _), root) in places.iter().zip(&roots) {
        values.push(super::list::read_nested(
            root.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: root.type_name().to_string(),
                span: Some(span),
            })?,
            accessors,
            span,
        )?);
    }
    for (index, ((accessors, name, escaped), root)) in places.iter().zip(roots).enumerate() {
        let updated = Value::list(super::list::update_nested(
            root.list_items().unwrap(),
            accessors,
            &values[(index + values.len() - 1) % values.len()],
            span,
        )?);
        if *escaped {
            runtime.set_or_define_exact_in(name, updated, environment, span)?;
        } else {
            runtime.set_or_define_in(name, updated, environment, span)?;
        }
    }
    stack.push(Value::Nil);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_shiftf_nested(
    places: &[(Vec<String>, String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    runtime: &Runtime,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() + 1 {
        return Err(invalid("shiftf has too few values on the stack", span));
    }
    let values = stack.split_off(stack.len() - places.len() - 1);
    let roots = &values[..places.len()];
    let old = places
        .iter()
        .zip(roots)
        .map(|((a, _, _), root)| super::list::read_nested(root.list_items().unwrap(), a, span))
        .collect::<Result<Vec<_>, _>>()?;
    let mut updated_roots: Vec<(&str, bool, Value)> = Vec::new();
    for (index, ((accessors, name, escaped), root)) in places.iter().zip(roots).enumerate() {
        let current_root = updated_roots
            .iter()
            .rev()
            .find(|(updated_name, updated_escaped, _)| {
                *updated_name == name && *updated_escaped == *escaped
            })
            .map(|(_, _, value)| value)
            .unwrap_or(root);
        let updated = Value::list(super::list::update_nested(
            current_root.list_items().unwrap(),
            accessors,
            old.get(index + 1).unwrap_or_else(|| values.last().unwrap()),
            span,
        )?);
        if *escaped {
            runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
        } else {
            runtime.set_or_define_in(name, updated.clone(), environment, span)?;
        }
        updated_roots.push((name, *escaped, updated));
    }
    stack.push(old[0].clone());
    *program_counter += 1;
    Ok(true)
}

#[allow(clippy::wildcard_imports)]
use super::super::*;

fn dynamic_nth_place_values(
    places: &[(Vec<String>, String, bool)],
    values: &[Value],
    span: Span,
) -> Result<Vec<Value>, RuntimeError> {
    places
        .iter()
        .zip(values.chunks_exact(2))
        .map(|((accessors, _, _), pair)| {
            let index = crate::builtins::index_argument("ROTATEF/SHIFTF NTH", &pair[0].primary_value())?;
            let elements = pair[1].list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".into(),
                actual: pair[1].type_name().into(),
                span: Some(span),
            })?;
            let target = if accessors.is_empty() {
                Value::list(elements.clone())
            } else {
                super::list::nested::read(elements.clone(), accessors, span)?
            };
            target
                .list_items()
                .and_then(|items| items.get(index).cloned())
                .ok_or_else(|| crate::builtins::out_of_bounds("ROTATEF/SHIFTF NTH", index))
        })
        .collect()
}

pub(super) fn execute_rotatef_nth_dynamic_places(
    places: &[(Vec<String>, String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    runtime: &Runtime,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let values = stack.split_off(stack.len().saturating_sub(places.len() * 2));
    if values.len() != places.len() * 2 {
        return Err(invalid("rotatef has too few values on the stack", span));
    }
    let old = dynamic_nth_place_values(places, &values, span)?;
    let mut updated_roots: Vec<(&str, bool, Value)> = Vec::new();
    for (index, ((accessors, name, escaped), pair)) in places.iter().zip(values.chunks_exact(2)).enumerate() {
        let current_root = updated_roots
            .iter()
            .rev()
            .find(|(updated_name, updated_escaped, _)| *updated_name == name && *updated_escaped == *escaped)
            .map(|(_, _, value)| value)
            .unwrap_or(&pair[1]);
        let list = current_root.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".into(),
            actual: current_root.type_name().into(),
            span: Some(span),
        })?;
        let nth = crate::builtins::index_argument("ROTATEF NTH", &pair[0].primary_value())?;
        let updated = Value::list(super::list::nested::update_dynamic(
            list, accessors, nth, &old[(index + old.len() - 1) % old.len()], span,
        )?);
        if *escaped { runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?; }
        else { runtime.set_or_define_in(name, updated.clone(), environment, span)?; }
        updated_roots.push((name, *escaped, updated));
    }
    stack.push(Value::Nil);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_shiftf_nth_dynamic_places(
    places: &[(Vec<String>, String, bool)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    runtime: &Runtime,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let values = stack.split_off(stack.len().saturating_sub(places.len() * 2 + 1));
    if values.len() != places.len() * 2 + 1 {
        return Err(invalid("shiftf has too few values on the stack", span));
    }
    let old = dynamic_nth_place_values(places, &values[..places.len() * 2], span)?;
    let mut updated_roots: Vec<(&str, bool, Value)> = Vec::new();
    for (index, ((accessors, name, escaped), pair)) in places.iter().zip(values[..places.len() * 2].chunks_exact(2)).enumerate() {
        let current_root = updated_roots
            .iter()
            .rev()
            .find(|(updated_name, updated_escaped, _)| *updated_name == name && *updated_escaped == *escaped)
            .map(|(_, _, value)| value)
            .unwrap_or(&pair[1]);
        let list = current_root.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "LIST".into(),
            actual: current_root.type_name().into(),
            span: Some(span),
        })?;
        let nth = crate::builtins::index_argument("SHIFTF NTH", &pair[0].primary_value())?;
        let value = old.get(index + 1).cloned().unwrap_or_else(|| values.last().unwrap().clone());
        let updated = Value::list(super::list::nested::update_dynamic(list, accessors, nth, &value, span)?);
        if *escaped { runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?; }
        else { runtime.set_or_define_in(name, updated.clone(), environment, span)?; }
        updated_roots.push((name, *escaped, updated));
    }
    stack.push(old[0].clone());
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_rotatef_nth_dynamic(
    accessors: &[String], name: &str, escaped: bool, stack: &mut Vec<Value>,
    environment: &Environment, runtime: &Runtime, program_counter: &mut usize, span: Span,
) -> Result<bool, RuntimeError> {
    let root = stack.pop().ok_or_else(|| invalid("rotatef NTH has no target", span))?;
    let index = stack.pop().ok_or_else(|| invalid("rotatef NTH has no index", span))?;
    let index = crate::builtins::index_argument("rotatef NTH", &index.primary_value())?;
    let elements = root.list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".into(), actual: root.type_name().into(), span: Some(span) })?;
    let old = if accessors.is_empty() {
        elements.get(index).cloned().ok_or_else(|| crate::builtins::out_of_bounds("rotatef NTH", index))?
    } else {
        super::list::nested::read(elements.clone(), accessors, span)?
            .list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".into(), actual: "not a list".into(), span: Some(span) })?
            .get(index).cloned().ok_or_else(|| crate::builtins::out_of_bounds("rotatef NTH", index))?
    };
    let updated = Value::list(super::list::nested::update_dynamic(elements, accessors, index, &old, span)?);
    if escaped { runtime.set_or_define_exact_in(name, updated, environment, span)?; } else { runtime.set_or_define_in(name, updated, environment, span)?; }
    stack.push(Value::Nil); *program_counter += 1; Ok(true)
}

pub(super) fn execute_shiftf_nth_dynamic(
    accessors: &[String], name: &str, escaped: bool, stack: &mut Vec<Value>,
    environment: &Environment, runtime: &Runtime, program_counter: &mut usize, span: Span,
) -> Result<bool, RuntimeError> {
    let new_value = stack.pop().ok_or_else(|| invalid("shiftf NTH has no value", span))?;
    let root = stack.pop().ok_or_else(|| invalid("shiftf NTH has no target", span))?;
    let index = stack.pop().ok_or_else(|| invalid("shiftf NTH has no index", span))?;
    let index = crate::builtins::index_argument("shiftf NTH", &index.primary_value())?;
    let elements = root.list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".into(), actual: root.type_name().into(), span: Some(span) })?;
    let target = if accessors.is_empty() { Value::list(elements.clone()) } else { super::list::nested::read(elements.clone(), accessors, span)? };
    let old = target.list_items().and_then(|items| items.get(index).cloned()).ok_or_else(|| crate::builtins::out_of_bounds("shiftf NTH", index))?;
    let updated = Value::list(super::list::nested::update_dynamic(elements, accessors, index, &new_value.primary_value(), span)?);
    if escaped { runtime.set_or_define_exact_in(name, updated, environment, span)?; } else { runtime.set_or_define_in(name, updated, environment, span)?; }
    stack.push(old); *program_counter += 1; Ok(true)
}

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
        values.push(super::list::nested::read(
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
        let updated = Value::list(super::list::nested::update(
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
        .map(|((a, _, _), root)| super::list::nested::read(root.list_items().unwrap(), a, span))
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
        let updated = Value::list(super::list::nested::update(
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

pub(super) fn execute_rotatef_mixed(
    places: &[ncl_compiler::RotateShiftPlace],
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
    let mut old = Vec::with_capacity(places.len());
    for (place, root) in places.iter().zip(&roots) {
        old.push(match place {
            ncl_compiler::RotateShiftPlace::Symbol(_, _) => root.clone(),
            ncl_compiler::RotateShiftPlace::NestedList(accessors, _, _) => {
                super::list::nested::read(
                    root.list_items().ok_or_else(|| RuntimeError::Type {
                        expected: "LIST".into(),
                        actual: root.type_name().into(),
                        span: Some(span),
                    })?,
                    accessors,
                    span,
                )?
            }
        });
    }
    for (index, (place, root)) in places.iter().zip(roots).enumerate() {
        let value = old[(index + old.len() - 1) % old.len()]
            .clone()
            .primary_value();
        match place {
            ncl_compiler::RotateShiftPlace::Symbol(name, escaped) => {
                if *escaped {
                    runtime.set_or_define_exact_in(name, value, environment, span)?
                } else {
                    runtime.set_or_define_in(name, value, environment, span)?
                }
            }
            ncl_compiler::RotateShiftPlace::NestedList(accessors, name, escaped) => {
                let updated = Value::list(super::list::nested::update(
                    root.list_items().unwrap(),
                    accessors,
                    &value,
                    span,
                )?);
                if *escaped {
                    runtime.set_or_define_exact_in(name, updated, environment, span)?
                } else {
                    runtime.set_or_define_in(name, updated, environment, span)?
                }
            }
        }
    }
    stack.push(Value::Nil);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_shiftf_mixed(
    places: &[ncl_compiler::RotateShiftPlace],
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
        .map(|(place, root)| match place {
            ncl_compiler::RotateShiftPlace::Symbol(_, _) => Ok(root.clone()),
            ncl_compiler::RotateShiftPlace::NestedList(accessors, _, _) => {
                super::list::nested::read(root.list_items().unwrap(), accessors, span)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (index, (place, root)) in places.iter().zip(roots).enumerate() {
        let value = values[index + 1].clone().primary_value();
        match place {
            ncl_compiler::RotateShiftPlace::Symbol(name, escaped) => {
                if *escaped {
                    runtime.set_or_define_exact_in(name, value, environment, span)?
                } else {
                    runtime.set_or_define_in(name, value, environment, span)?
                }
            }
            ncl_compiler::RotateShiftPlace::NestedList(accessors, name, escaped) => {
                let updated = Value::list(super::list::nested::update(
                    root.list_items().unwrap(),
                    accessors,
                    &value,
                    span,
                )?);
                if *escaped {
                    runtime.set_or_define_exact_in(name, updated, environment, span)?
                } else {
                    runtime.set_or_define_in(name, updated, environment, span)?
                }
            }
        }
    }
    stack.push(old[0].clone());
    *program_counter += 1;
    Ok(true)
}

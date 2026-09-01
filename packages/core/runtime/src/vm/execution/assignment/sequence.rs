#[allow(clippy::wildcard_imports)]
use super::super::*;
use super::rotate_shift;

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
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
            let index = *index;
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
        Instruction::ModifyNthDynamic { arithmetic, name, escaped } => {
            let delta = stack.pop().ok_or_else(|| invalid("modify nth has no delta", span))?.primary_value();
            let current = stack.pop().ok_or_else(|| invalid("modify nth has no target", span))?.primary_value();
            let index = stack.pop().ok_or_else(|| invalid("modify nth has no index", span))?.primary_value();
            let index = crate::builtins::index_argument("modify nth", &index)?;
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".to_string(), actual: current.type_name().to_string(), span: Some(span) })?;
            let slot = elements.get(index).ok_or_else(|| crate::builtins::out_of_bounds("modify nth", index))?.clone();
            let value = runtime.apply_in(&Value::symbol(arithmetic.clone()), &[slot, delta], span, environment)?.primary_value();
            *elements.get_mut(index).ok_or_else(|| crate::builtins::out_of_bounds("modify nth", index))? = value.clone();
            let updated = Value::list(elements);
            if *escaped { runtime.set_or_define_exact_in(name, updated, environment, span)?; } else { runtime.set_or_define_in(name, updated, environment, span)?; }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::ListMutationNthDynamic { operator, name, escaped } => {
            let current = stack.pop().ok_or_else(|| invalid("nth mutation has no target", span))?.primary_value();
            let index = stack.pop().ok_or_else(|| invalid("nth mutation has no index", span))?.primary_value();
            let index = crate::builtins::index_argument("nth mutation", &index)?;
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".to_string(), actual: current.type_name().to_string(), span: Some(span) })?;
            let slot = elements.get(index).ok_or_else(|| crate::builtins::out_of_bounds("nth mutation", index))?.clone();
            let value = if operator == "PUSH" {
                let value = stack.pop().ok_or_else(|| invalid("nth PUSH has no value", span))?.primary_value();
                let mut slot = slot.list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".to_string(), actual: slot.type_name().to_string(), span: Some(span) })?;
                slot.insert(0, value);
                Value::list(slot)
            } else {
                let mut slot = slot.list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".to_string(), actual: slot.type_name().to_string(), span: Some(span) })?;
                let popped = slot.first().cloned().unwrap_or(Value::Nil);
                if !slot.is_empty() { slot.remove(0); }
                let updated = Value::list(slot);
                elements[index] = updated;
                let outer = Value::list(elements);
                if *escaped { runtime.set_or_define_exact_in(name, outer, environment, span)?; } else { runtime.set_or_define_in(name, outer, environment, span)?; }
                stack.push(popped);
                *program_counter += 1;
                return Ok(true);
            };
            elements[index] = value.clone();
            let updated = Value::list(elements);
            if *escaped { runtime.set_or_define_exact_in(name, updated, environment, span)?; } else { runtime.set_or_define_in(name, updated, environment, span)?; }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::ListMutationNthPushNew { name, escaped } => {
            let current = stack.pop().ok_or_else(|| invalid("nth PUSHNEW has no target", span))?.primary_value();
            let index = stack.pop().ok_or_else(|| invalid("nth PUSHNEW has no index", span))?.primary_value();
            let value = stack.pop().ok_or_else(|| invalid("nth PUSHNEW has no value", span))?.primary_value();
            let index = crate::builtins::index_argument("nth PUSHNEW", &index)?;
            let mut elements = current.list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".to_string(), actual: current.type_name().to_string(), span: Some(span) })?;
            let slot = elements.get(index).ok_or_else(|| crate::builtins::out_of_bounds("nth PUSHNEW", index))?.clone();
            let mut slot = slot.list_items().ok_or_else(|| RuntimeError::Type { expected: "LIST".to_string(), actual: slot.type_name().to_string(), span: Some(span) })?;
            let changed = !slot.iter().any(|candidate| crate::builtins::type_predicates::eql_value(&value, candidate));
            if changed {
                slot.insert(0, value);
            }
            let result = Value::list(slot);
            if changed {
                elements[index] = result.clone();
                let updated = Value::list(elements);
                if *escaped { runtime.set_or_define_exact_in(name, updated, environment, span)?; } else { runtime.set_or_define_in(name, updated, environment, span)?; }
            }
            stack.push(result);
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
        Instruction::RotatefSymbols(places) => rotate_shift::execute_rotatef(
            runtime,
            places,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::ShiftfSymbols(places) => {
            rotate_shift::execute_shiftf(runtime, places, stack, environment, program_counter, span)
        }
        Instruction::RotatefNestedList(places) => rotate_shift::execute_rotatef_nested(
            places,
            stack,
            environment,
            runtime,
            program_counter,
            span,
        ),
        Instruction::ShiftfNestedList(places) => rotate_shift::execute_shiftf_nested(
            places,
            stack,
            environment,
            runtime,
            program_counter,
            span,
        ),
        Instruction::RotatefMixed(places) => rotate_shift::execute_rotatef_mixed(
            places,
            stack,
            environment,
            runtime,
            program_counter,
            span,
        ),
        Instruction::ShiftfMixed(places) => rotate_shift::execute_shiftf_mixed(
            places,
            stack,
            environment,
            runtime,
            program_counter,
            span,
        ),
        _ => Ok(false),
    }
}

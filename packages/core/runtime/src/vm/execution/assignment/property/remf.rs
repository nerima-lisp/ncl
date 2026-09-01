#[allow(clippy::wildcard_imports)]
use super::super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let Instruction::Remf { name, escaped } = instruction else {
        return Ok(false);
    };
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

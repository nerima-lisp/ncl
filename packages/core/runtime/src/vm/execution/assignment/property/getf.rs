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
    let Instruction::SetfGetfDynamic { name, escaped } = instruction else {
        return Ok(false);
    };
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf getf has no value on the stack", span))?
        .primary_value();
    let indicator = stack
        .pop()
        .ok_or_else(|| invalid("setf getf has no indicator on the stack", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf getf has no target on the stack", span))?
        .primary_value();
    let mut properties = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    if !properties.len().is_multiple_of(2) {
        return Err(invalid("GETF needs an even property list", span));
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
    let updated = Value::list(properties);
    if *escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

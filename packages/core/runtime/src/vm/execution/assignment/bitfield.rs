use super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let (operator, binding) = match instruction {
        Instruction::SetfBitfieldDynamic { operator, name, escaped } =>
            (operator, Some((name, escaped))),
        Instruction::SetfBitfieldValue { operator } => {
            let value = stack.pop().ok_or_else(|| invalid("setf bitfield has no value on the stack", span))?.primary_value();
            let old_value = stack.pop().ok_or_else(|| invalid("setf bitfield has no target on the stack", span))?.primary_value();
            let byte_spec = stack.pop().ok_or_else(|| invalid("setf bitfield has no byte specifier on the stack", span))?.primary_value();
            let _updated = bitfield_value(operator, value.clone(), byte_spec, old_value, span)?;
            stack.push(value);
            *program_counter += 1;
            return Ok(true);
        }
        _ => return Ok(false),
    };
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf bitfield has no value on the stack", span))?
        .primary_value();
    let old_value = stack
        .pop()
        .ok_or_else(|| invalid("setf bitfield has no target on the stack", span))?
        .primary_value();
    let byte_spec = stack
        .pop()
        .ok_or_else(|| invalid("setf bitfield has no byte specifier on the stack", span))?
        .primary_value();
    let updated = bitfield_value(operator, value.clone(), byte_spec, old_value, span)?;
    if let Some((name, escaped)) = binding {
      if *escaped { runtime.set_or_define_exact_in(name, updated, environment, span)?;
      } else { runtime.set_or_define_in(name, updated, environment, span)?; }
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

fn bitfield_value(operator: &str, value: Value, byte_spec: Value, old_value: Value, span: Span) -> Result<Value, RuntimeError> {
    match operator {
        "LDB" => crate::builtins::dpb(&[value, byte_spec, old_value]),
        "MASK-FIELD" => crate::builtins::deposit_field(&[value, byte_spec, old_value]),
        _ => Err(invalid("unsupported SETF bitfield operator", span)),
    }
}

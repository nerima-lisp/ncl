use super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let Instruction::SetfBitfieldDynamic {
        operator,
        name,
        escaped,
    } = instruction
    else {
        return Ok(false);
    };
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf bitfield has no value on the stack", span))?
        .primary_value();
    let byte_spec = stack
        .pop()
        .ok_or_else(|| invalid("setf bitfield has no byte specifier on the stack", span))?
        .primary_value();
    let old_value = stack
        .pop()
        .ok_or_else(|| invalid("setf bitfield has no target on the stack", span))?
        .primary_value();
    let updated = match operator.as_str() {
        "LDB" => crate::builtins::dpb(&[value.clone(), byte_spec, old_value])?,
        "MASK-FIELD" => crate::builtins::deposit_field(&[value.clone(), byte_spec, old_value])?,
        _ => return Err(invalid("unsupported SETF bitfield operator", span)),
    };
    if *escaped {
        runtime.set_or_define_exact_in(name, updated, environment, span)?;
    } else {
        runtime.set_or_define_in(name, updated, environment, span)?;
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

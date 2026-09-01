#[allow(clippy::wildcard_imports)]
use super::super::super::*;

pub(super) fn execute(
    _runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    _environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::SetfSlotValueDynamic => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf slot-value has no value on the stack", span))?
                .primary_value();
            let slot = stack
                .pop()
                .ok_or_else(|| invalid("setf slot-value has no slot on the stack", span))?
                .primary_value();
            let instance = stack
                .pop()
                .ok_or_else(|| invalid("setf slot-value has no instance on the stack", span))?
                .primary_value();
            let slot_name = Runtime::slot_name_from_value(&slot, span)?;
            let Some(class) = instance.instance_class_definition() else {
                return Err(RuntimeError::Type {
                    expected: "STANDARD-OBJECT".to_string(),
                    actual: instance.type_name().to_string(),
                    span: Some(span),
                });
            };
            if !instance.set_instance_slot(&class.name, &slot_name, value.clone()) {
                return Err(invalid("slot is not defined for this class", span));
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

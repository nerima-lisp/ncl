#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::SetfGetfDynamic { name, escaped } => {
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
        Instruction::SetfGetDynamic => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf get has no value on the stack", span))?
                .primary_value();
            let indicator = stack
                .pop()
                .ok_or_else(|| invalid("setf get has no indicator on the stack", span))?
                .primary_value();
            let symbol = stack
                .pop()
                .ok_or_else(|| invalid("setf get has no target on the stack", span))?
                .primary_value();
            if symbol.symbol_reference().is_none() {
                return Err(invalid("setf get target must be a symbol", span));
            }
            let plist = environment.symbol_plist(&symbol).unwrap_or(Value::Nil);
            let mut properties = plist.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: plist.type_name().to_string(),
                span: Some(span),
            })?;
            if !properties.len().is_multiple_of(2) {
                return Err(invalid("SETF GET needs an even property list", span));
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
            environment.set_symbol_plist(&symbol, Value::list(properties));
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfGethashDynamic => {
            let value = stack
                .pop()
                .ok_or_else(|| invalid("setf gethash has no value on the stack", span))?
                .primary_value();
            let table = stack
                .pop()
                .ok_or_else(|| invalid("setf gethash has no table on the stack", span))?
                .primary_value();
            let key = stack
                .pop()
                .ok_or_else(|| invalid("setf gethash has no key on the stack", span))?
                .primary_value();
            let test = table.hash_table_test().ok_or_else(|| RuntimeError::Type {
                expected: "HASH-TABLE".to_string(),
                actual: table.type_name().to_string(),
                span: Some(span),
            })?;
            let entries = table
                .hash_table_entries()
                .ok_or_else(|| RuntimeError::Type {
                    expected: "HASH-TABLE".to_string(),
                    actual: table.type_name().to_string(),
                    span: Some(span),
                })?;
            let mut entries = entries.borrow_mut();
            if let Some((_, slot)) = entries.iter_mut().find(|(stored_key, _)| {
                crate::builtins::hash_table_key_equal(test, stored_key, &key)
            }) {
                *slot = value.clone();
            } else {
                entries.push((key, value.clone()));
            }
            stack.push(value);
            *program_counter += 1;
            Ok(true)
        }
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

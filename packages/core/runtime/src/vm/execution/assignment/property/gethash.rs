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
        _ => Ok(false),
    }
}

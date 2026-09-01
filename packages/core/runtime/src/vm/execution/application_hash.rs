#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_hash_table_instruction(
    stack: &mut Vec<Value>, operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("hash-table operation has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = match operation {
        "GETHASH" => crate::builtins::gethash(&arguments),
        "REMHASH" => crate::builtins::remhash(&arguments),
        "MAKE-HASH-TABLE" => crate::builtins::make_hash_table(&arguments),
        "CLRHASH" => crate::builtins::clrhash(&arguments),
        "HASH-TABLE-COUNT" => crate::builtins::hash_table_count(&arguments),
        "HASH-TABLE-TEST" => crate::builtins::hash_table_test_value(&arguments),
        "NCL-HASH-TABLE-KEYS" => crate::builtins::hash_table_keys(&arguments),
        "NCL-HASH-TABLE-VALUES" => crate::builtins::hash_table_values(&arguments),
        _ => Err(invalid("unknown hash-table operation", span)),
    }?;
    stack.push(value);
    Ok(())
}

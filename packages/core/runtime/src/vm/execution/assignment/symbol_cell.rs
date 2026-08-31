#[allow(clippy::wildcard_imports)]
use super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    operator: &str,
    stack: &mut Vec<Value>,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf symbol cell has no value on the stack", span))?
        .primary_value();
    let symbol = stack
        .pop()
        .ok_or_else(|| invalid("setf symbol cell has no target on the stack", span))?
        .primary_value();
    let (name, exact) = symbol
        .symbol_reference()
        .ok_or_else(|| invalid("setf symbol cell target must be a symbol", span))?;
    match operator {
        "SYMBOL-VALUE" => {
            runtime.ensure_symbol_writable(name, exact, span)?;
            if exact {
                runtime.set_symbol_value_exact(name, value.clone());
            } else {
                runtime.set_symbol_value(name, value.clone());
            }
        }
        "SYMBOL-FUNCTION" => {
            if !matches!(&value, Value::Function(_)) {
                return Err(RuntimeError::Type {
                    expected: "FUNCTION".to_string(),
                    actual: value.type_name().to_string(),
                    span: Some(span),
                });
            }
            runtime.set_symbol_function(name, exact, value.clone());
        }
        _ => unreachable!("unsupported symbol cell operator"),
    }
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

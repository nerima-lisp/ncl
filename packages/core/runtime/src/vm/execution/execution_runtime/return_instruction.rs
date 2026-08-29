use ncl_compiler::Instruction;
use ncl_syntax::Span;

use crate::vm::primitives::{invalid, pop_value};
use crate::{Environment, ReturnValue, RuntimeError, Value};

pub(super) fn execute_return_instruction(
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    scopes: &[(Environment, usize, usize)],
    span: Span,
) -> Result<Option<Value>, RuntimeError> {
    match instruction {
        Instruction::ReturnFrom { name } => {
            let value = pop_value(stack, span, "return-from")?;
            Err(RuntimeError::ReturnFrom {
                block: name.clone(),
                target: environment.lookup_block(name),
                value: ReturnValue::new(value),
                span: Some(span),
            })
        }
        Instruction::Return => {
            if !scopes.is_empty() {
                return Err(invalid(
                    "compiled function returned with an open scope",
                    span,
                ));
            }
            Ok(Some(pop_value(stack, span, "return")?))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use ncl_compiler::Instruction;
    use ncl_syntax::Span;

    use super::execute_return_instruction;
    use crate::{Environment, ReturnValue, RuntimeError, Value};

    #[test]
    fn rejects_return_with_an_open_scope() {
        let span = Span::new(0, 1);
        let error = match execute_return_instruction(
            &Instruction::Return,
            &mut vec![Value::Nil],
            &Environment::new(),
            &[(Environment::new(), 0, 0)],
            span,
        ) {
            Err(error) => error,
            Ok(value) => panic!("unexpected successful result: {value:?}"),
        };
        assert!(
            matches!(error, RuntimeError::InvalidForm { message, .. } if message == "compiled function returned with an open scope")
        );
    }

    #[test]
    fn returns_the_top_value_when_the_scope_is_closed() {
        let span = Span::new(0, 1);
        let result = execute_return_instruction(
            &Instruction::Return,
            &mut vec![Value::Integer(42)],
            &Environment::new(),
            &[],
            span,
        );

        assert!(matches!(result, Ok(Some(Value::Integer(42)))));
    }

    #[test]
    fn return_instructions_reject_missing_values() {
        let environment = Environment::new();
        let span = Span::new(0, 1);
        let cases = [
            (Instruction::Return, "return has no value on the stack"),
            (
                Instruction::ReturnFrom {
                    name: "DONE".to_string(),
                },
                "return-from has no value on the stack",
            ),
        ];

        for (instruction, expected) in cases {
            let result =
                execute_return_instruction(&instruction, &mut Vec::new(), &environment, &[], span)
                    .map(|_| ());
            assert!(
                matches!(result, Err(RuntimeError::InvalidForm { message, .. }) if message == expected)
            );
        }
    }

    #[test]
    fn non_return_instructions_leave_return_processing_unchanged() {
        let result = execute_return_instruction(
            &Instruction::Pop,
            &mut vec![Value::Integer(7)],
            &Environment::new(),
            &[],
            Span::new(0, 1),
        );

        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn return_from_preserves_the_block_target_and_value() {
        let span = Span::new(0, 1);
        let environment = Environment::new();
        let result = execute_return_instruction(
            &Instruction::ReturnFrom {
                name: "DONE".to_string(),
            },
            &mut vec![Value::Integer(7)],
            &environment,
            &[],
            span,
        );

        assert!(matches!(
            result,
            Err(RuntimeError::ReturnFrom {
                block,
                target: None,
                value,
                ..
            }) if block == "DONE" && value == ReturnValue::new(Value::Integer(7))
        ));
    }
}

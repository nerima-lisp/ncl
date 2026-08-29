use ncl_compiler::Instruction;
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::execution::execute_stack_instruction;

#[test]
fn executes_stack_transformations_as_a_table() {
    let runtime = Runtime::new();
    let span = Span::new(0, 1);
    let cases = [
        (Instruction::Pop, vec![Value::Integer(1)], vec![]),
        (
            Instruction::Dup,
            vec![Value::Integer(1)],
            vec![Value::Integer(1), Value::Integer(1)],
        ),
        (
            Instruction::Primary,
            vec![Value::values(vec![Value::Integer(1), Value::Integer(2)])],
            vec![Value::Integer(1)],
        ),
        (
            Instruction::Values(2),
            vec![Value::Integer(1), Value::Integer(2)],
            vec![Value::values(vec![Value::Integer(1), Value::Integer(2)])],
        ),
        (
            Instruction::MultipleValueList,
            vec![Value::values(vec![Value::Integer(1), Value::Integer(2)])],
            vec![Value::list(vec![Value::Integer(1), Value::Integer(2)])],
        ),
    ];

    for (instruction, mut stack, expected_stack) in cases {
        let mut scopes = Vec::new();
        let mut environment = Environment::new();
        let mut program_counter = 0;
        let result = execute_stack_instruction(
            &runtime,
            &instruction,
            &mut stack,
            &mut scopes,
            &mut environment,
            &mut program_counter,
            span,
        );
        assert!(matches!(result, Ok(true)));
        assert_eq!(
            stack.iter().map(Value::to_string).collect::<Vec<_>>(),
            expected_stack
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(program_counter, 1);
    }
}

#[test]
fn rejects_invalid_stack_transformations_as_a_table() {
    let runtime = Runtime::new();
    let span = Span::new(0, 1);
    let cases = [
        (Instruction::Pop, "pop has no value on the stack"),
        (Instruction::Dup, "dup has no value on the stack"),
        (
            Instruction::Primary,
            "primary value has no value on the stack",
        ),
        (Instruction::Values(1), "values has too few stack values"),
        (
            Instruction::MultipleValueList,
            "multiple-value-list has no value on the stack",
        ),
        (Instruction::ExitScope, "scope exit has no matching scope"),
    ];

    for (instruction, message) in cases {
        let mut stack = Vec::new();
        let mut scopes = Vec::new();
        let mut environment = Environment::new();
        let mut program_counter = 0;
        let result = execute_stack_instruction(
            &runtime,
            &instruction,
            &mut stack,
            &mut scopes,
            &mut environment,
            &mut program_counter,
            span,
        );
        let result_debug = format!("{result:?}");
        assert!(
            matches!(result, Err(RuntimeError::InvalidForm { message: actual, .. }) if actual == message),
            "{instruction:?}: {result_debug}"
        );
        assert_eq!(program_counter, 0);
    }
}

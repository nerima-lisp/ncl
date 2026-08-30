use super::*;

#[test]
fn set_instructions_store_primary_values_and_advance() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    for (instruction, expected_name) in [
        (Instruction::Set("x".to_string()), "x"),
        (Instruction::SetExact("Foo".to_string()), "Foo"),
    ] {
        let mut program_counter = 2;
        let mut stack = vec![Value::values(vec![Value::Integer(7), Value::Integer(8)])];
        let result = execute_set_instruction(
            &runtime,
            &instruction,
            &mut stack,
            &environment,
            &mut program_counter,
            span,
        );
        assert!(matches!(result, Ok(true)));
        assert_eq!(program_counter, 3);
        assert!(matches!(stack.as_slice(), [Value::Integer(7)]));
        let stored = if expected_name == "Foo" {
            environment.lookup_exact(expected_name)
        } else {
            environment.lookup(expected_name)
        };
        assert!(matches!(stored, Some(Value::Integer(7))));
    }
}

#[test]
fn set_instruction_rejects_a_missing_stack_value() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 0;
    assert_invalid(
        execute_set_instruction(
            &runtime,
            &Instruction::Set("x".to_string()),
            &mut [],
            &environment,
            &mut program_counter,
            span,
        ),
        "setq has no value on the stack",
    );
}

#[test]
fn unhandled_assignment_instruction_is_reported_as_not_executed() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 4;
    let mut stack = Vec::new();
    assert!(matches!(
        execute_set_instruction(
            &runtime,
            &Instruction::EnterScope,
            &mut stack,
            &environment,
            &mut program_counter,
            span
        ),
        Ok(false)
    ));
    assert_eq!(program_counter, 4);
}

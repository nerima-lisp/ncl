use super::*;

#[test]
fn setf_instructions_reject_a_missing_stack_value() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let place = Form::atom("x", span);
    let mut program_counter = 0;

    assert_invalid(
        execute_set_instruction(
            &runtime,
            &Instruction::Setf(place.clone()),
            &mut Vec::new(),
            &environment,
            &mut program_counter,
            span,
        ),
        "setf has no value on the stack",
    );
    assert_invalid(
        execute_set_instruction(
            &runtime,
            &Instruction::MapIntoSetf(place),
            &mut Vec::new(),
            &environment,
            &mut program_counter,
            span,
        ),
        "map-into has no value on the stack",
    );
    assert_eq!(program_counter, 0);
}

#[test]
fn parallel_set_instructions_reject_fewer_values_than_targets() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 0;

    let mut stack = vec![Value::Integer(1)];
    assert_invalid(
        execute_parallel_set_instruction(
            &runtime,
            &Instruction::Psetq(vec!["a".to_string(), "b".to_string()]),
            &mut stack,
            &environment,
            &mut program_counter,
            span,
        ),
        "psetq has fewer values than targets",
    );

    let mut stack = vec![Value::Integer(1)];
    assert_invalid(
        execute_parallel_set_instruction(
            &runtime,
            &Instruction::PsetqExact(vec![("a".to_string(), false), ("b".to_string(), false)]),
            &mut stack,
            &environment,
            &mut program_counter,
            span,
        ),
        "psetq has fewer values than targets",
    );
    assert_eq!(program_counter, 0);
}

#[test]
fn psetf_symbols_rejects_fewer_values_than_targets() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 0;
    let mut stack = vec![Value::Integer(1)];

    assert_invalid(
        execute_parallel_set_instruction(
            &runtime,
            &Instruction::PsetfSymbols(vec![("a".to_string(), false), ("b".to_string(), false)]),
            &mut stack,
            &environment,
            &mut program_counter,
            span,
        ),
        "psetf has fewer values than targets",
    );
    assert_eq!(program_counter, 0);
}

#[test]
fn psetq_stores_primary_values_and_pushes_nil() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 0;
    let mut stack = vec![
        Value::values(vec![Value::Integer(1), Value::Integer(9)]),
        Value::Integer(2),
    ];

    let result = execute_parallel_set_instruction(
        &runtime,
        &Instruction::Psetq(vec!["a".to_string(), "b".to_string()]),
        &mut stack,
        &environment,
        &mut program_counter,
        span,
    );

    assert!(matches!(result, Ok(true)));
    assert_eq!(program_counter, 1);
    assert!(matches!(stack.as_slice(), [Value::Nil]));
    assert!(matches!(environment.lookup("a"), Some(Value::Integer(1))));
    assert!(matches!(environment.lookup("b"), Some(Value::Integer(2))));
}

#[test]
fn multiple_value_setq_stores_missing_values_as_nil() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 0;
    let mut stack = vec![Value::values(vec![Value::Integer(3)])];

    let result = execute_parallel_set_instruction(
        &runtime,
        &Instruction::MultipleValueSetq(vec!["a".to_string(), "b".to_string()]),
        &mut stack,
        &environment,
        &mut program_counter,
        span,
    );

    assert!(matches!(result, Ok(true)));
    assert_eq!(program_counter, 1);
    assert!(matches!(stack.as_slice(), [Value::Integer(3)]));
    assert!(matches!(environment.lookup("a"), Some(Value::Integer(3))));
    assert!(matches!(environment.lookup("b"), Some(Value::Nil)));
}

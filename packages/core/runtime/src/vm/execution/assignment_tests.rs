#[allow(clippy::wildcard_imports)]
use super::*;

use ncl_syntax::Form;

fn assert_invalid(result: Result<bool, RuntimeError>, expected: &str) {
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. }) if message == expected
    ));
}

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
            &mut [],
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
            &mut [],
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
fn psetq_exact_binds_escaped_and_normalized_names() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 3;
    let mut stack = vec![Value::Integer(1), Value::Integer(2)];
    let instruction =
        Instruction::PsetqExact(vec![("Foo".to_string(), true), ("bar".to_string(), false)]);

    let result = execute_parallel_set_instruction(
        &runtime,
        &instruction,
        &mut stack,
        &environment,
        &mut program_counter,
        span,
    );

    assert!(matches!(result, Ok(true)));
    assert_eq!(program_counter, 4);
    assert!(matches!(stack.as_slice(), [Value::Nil]));
    assert!(matches!(
        environment.lookup_exact("Foo"),
        Some(Value::Integer(1))
    ));
    assert!(environment.lookup_exact("foo").is_none());
    assert!(matches!(environment.lookup("bar"), Some(Value::Integer(2))));
}

#[test]
fn multiple_value_setq_exact_binds_escaped_and_normalized_names_and_pushes_the_primary_value() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 5;
    let mut stack = vec![Value::values(vec![Value::Integer(10), Value::Integer(20)])];
    let instruction = Instruction::MultipleValueSetqExact(vec![
        ("Foo".to_string(), true),
        ("bar".to_string(), false),
    ]);

    let result = execute_parallel_set_instruction(
        &runtime,
        &instruction,
        &mut stack,
        &environment,
        &mut program_counter,
        span,
    );

    assert!(matches!(result, Ok(true)));
    assert_eq!(program_counter, 6);
    assert_eq!(stack.len(), 1);
    assert_eq!(stack[0].to_string(), "10");
    assert!(matches!(
        environment.lookup_exact("Foo"),
        Some(Value::Integer(10))
    ));
    assert!(matches!(
        environment.lookup("bar"),
        Some(Value::Integer(20))
    ));
}

#[test]
fn multiple_value_setq_exact_defaults_missing_values_to_nil() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 0;
    let mut stack = vec![Value::values(vec![Value::Integer(1)])];
    let instruction = Instruction::MultipleValueSetqExact(vec![
        ("a".to_string(), false),
        ("b".to_string(), false),
    ]);

    let result = execute_parallel_set_instruction(
        &runtime,
        &instruction,
        &mut stack,
        &environment,
        &mut program_counter,
        span,
    );

    assert!(matches!(result, Ok(true)));
    assert!(matches!(environment.lookup("a"), Some(Value::Integer(1))));
    assert!(matches!(environment.lookup("b"), Some(Value::Nil)));
}

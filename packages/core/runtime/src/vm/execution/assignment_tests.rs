#[allow(clippy::wildcard_imports)]
use super::*;

use ncl_syntax::Form;

mod stack_validation;

fn assert_invalid(result: Result<bool, RuntimeError>, expected: &str) {
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. }) if message == expected
    ));
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
fn psetf_symbols_assigns_after_values_and_returns_last_value() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut program_counter = 3;
    let mut stack = vec![Value::Integer(1), Value::Integer(2)];
    let instruction = Instruction::PsetfSymbols(vec![
        ("first".to_string(), false),
        ("second".to_string(), false),
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
    assert_eq!(program_counter, 4);
    assert!(matches!(stack.as_slice(), [Value::Integer(2)]));
    assert!(matches!(
        environment.lookup("first"),
        Some(Value::Integer(1))
    ));
    assert!(matches!(
        environment.lookup("second"),
        Some(Value::Integer(2))
    ));
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
mod basic;

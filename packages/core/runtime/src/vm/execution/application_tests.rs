#[allow(clippy::wildcard_imports)]
use super::*;

fn assert_invalid(result: Result<(), RuntimeError>, expected: &str) {
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. }) if message == expected
    ));
}

#[test]
fn stack_operations_reject_invalid_shapes() {
    type StackOperation =
        fn(&Runtime, usize, &mut Vec<Value>, &Environment, Span) -> Result<(), RuntimeError>;

    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let cases: [(&str, StackOperation, &str); 3] = [
        (
            "call",
            execute_call_instruction,
            "call has too few stack values",
        ),
        (
            "apply",
            execute_apply_instruction,
            "apply has too few stack values",
        ),
        (
            "mapcar",
            execute_mapcar_instruction,
            "mapcar has too few stack values",
        ),
    ];

    for (name, operation, expected) in cases {
        let mut stack = Vec::new();
        assert_invalid(
            operation(&runtime, 0, &mut stack, &environment, span),
            expected,
        );
        assert!(!name.is_empty());
    }
}

#[test]
fn stack_operations_reject_invalid_sequence_shapes() {
    type StackOperation =
        fn(&Runtime, usize, &mut Vec<Value>, &Environment, Span) -> Result<(), RuntimeError>;

    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let cases: [(&str, StackOperation, Vec<Value>, &str); 2] = [
        (
            "apply",
            execute_apply_instruction,
            vec![Value::Nil, Value::Integer(1)],
            "apply's final argument must be a proper list",
        ),
        (
            "mapcar",
            execute_mapcar_instruction,
            vec![Value::Nil, Value::Integer(1)],
            "mapcar arguments must be proper lists",
        ),
    ];

    for (name, operation, mut stack, expected) in cases {
        assert_invalid(
            operation(&runtime, 1, &mut stack, &environment, span),
            expected,
        );
        assert!(!name.is_empty());
    }
}

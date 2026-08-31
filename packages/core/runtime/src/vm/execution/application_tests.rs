#[allow(clippy::wildcard_imports)]
use super::*;

use ncl_compiler::Constant;

fn constant_function(value: i64) -> FunctionCode {
    FunctionCode {
        name: None,
        parameters: Vec::new(),
        required_escaped: Vec::new(),
        optional: Vec::new(),
        keywords: Vec::new(),
        has_keyword_section: false,
        allow_other_keys: false,
        rest: None,
        rest_escaped: false,
        auxiliary: Vec::new(),
        instructions: vec![
            Instruction::Constant(Constant::Integer(value)),
            Instruction::Return,
        ],
    }
}

fn identity_function() -> FunctionCode {
    FunctionCode {
        name: None,
        parameters: vec!["x".to_string()],
        required_escaped: vec![false],
        optional: Vec::new(),
        keywords: Vec::new(),
        has_keyword_section: false,
        allow_other_keys: false,
        rest: None,
        rest_escaped: false,
        auxiliary: Vec::new(),
        instructions: vec![Instruction::Load("x".to_string()), Instruction::Return],
    }
}

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
            "mapcar arguments must be sequences",
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

#[test]
fn apply_instruction_calls_the_function_with_the_expanded_final_list() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![constant_function(42)],
        entry: 0,
    });
    let function_value = Value::compiled(program, 0, environment.clone());
    let mut stack = vec![function_value, Value::Nil];

    let result = execute_apply_instruction(&runtime, 1, &mut stack, &environment, span);

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(42)));
}

#[test]
fn mapcar_instruction_applies_the_function_across_the_sequence() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![identity_function()],
        entry: 0,
    });
    let function_value = Value::compiled(program, 0, environment.clone());
    let sequence = Value::list(vec![
        Value::Integer(1),
        Value::Integer(2),
        Value::Integer(3),
    ]);
    let mut stack = vec![function_value, sequence];

    let result = execute_mapcar_instruction(&runtime, 1, &mut stack, &environment, span);

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert_eq!(stack[0].to_string(), "(1 2 3)");
}

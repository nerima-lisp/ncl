use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError, Value};

use super::execution::{
    execute_apply_instruction, execute_call_instruction, execute_list_mapping_instruction,
    execute_multiple_value_call_instruction,
};

#[test]
fn rejects_call_without_enough_stack_values() {
    let result = execute_call_instruction(
        &Runtime::new(),
        0,
        &mut Vec::new(),
        &Environment::new(),
        Span::new(0, 1),
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "call has too few stack values"
    ));
}

#[test]
fn rejects_apply_without_enough_stack_values_or_a_final_list() {
    let mut stack = Vec::new();
    let result = execute_apply_instruction(
        &Runtime::new(),
        0,
        &mut stack,
        &Environment::new(),
        Span::new(0, 1),
    );
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "apply has too few stack values"
    ));

    stack = vec![Value::Integer(1), Value::Integer(2)];
    let result = execute_apply_instruction(
        &Runtime::new(),
        1,
        &mut stack,
        &Environment::new(),
        Span::new(0, 1),
    );
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "apply's final argument must be a proper list"
    ));
}

#[test]
fn rejects_mapcar_without_enough_stack_values_or_proper_lists() {
    let result = execute_list_mapping_instruction(
        &Runtime::new(),
        "MAPCAR",
        0,
        &mut Vec::new(),
        &Environment::new(),
        Span::new(0, 1),
    );
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "mapcar has too few stack values"
    ));

    let mut stack = vec![Value::Integer(1), Value::Integer(2)];
    let result = execute_list_mapping_instruction(
        &Runtime::new(),
        "MAPCAR",
        1,
        &mut stack,
        &Environment::new(),
        Span::new(0, 1),
    );
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "mapcar arguments must be proper lists"
    ));
}

#[test]
fn rejects_multiple_value_call_without_enough_stack_values() {
    let result = execute_multiple_value_call_instruction(
        &Runtime::new(),
        0,
        &mut Vec::new(),
        &Environment::new(),
        Span::new(0, 1),
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "multiple-value-call has too few stack values"
    ));
}

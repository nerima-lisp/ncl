use std::rc::Rc;

use ncl_compiler::{Instruction, Program};
use ncl_syntax::Span;

use crate::vm::run;
use crate::{Environment, Runtime, RuntimeError, Value};

use super::function;

#[test]
fn rejects_too_few_required_arguments() {
    let runtime = Runtime::new();
    let mut compiled = function(vec![Instruction::Return]);
    compiled.parameters.push("value".to_string());
    let program = Rc::new(Program {
        functions: vec![compiled],
        entry: 0,
    });

    let result = run(
        &runtime,
        &program,
        0,
        &Environment::new(),
        &[],
        Span::new(0, 1),
    );

    assert!(
        matches!(result, Err(RuntimeError::Arity { expected, actual, .. }) if expected == "1" && actual == 0)
    );
}

#[test]
fn rejects_too_many_arguments_without_a_rest_parameter() {
    let runtime = Runtime::new();
    let mut compiled = function(vec![Instruction::Return]);
    compiled.parameters.push("value".to_string());
    let program = Rc::new(Program {
        functions: vec![compiled],
        entry: 0,
    });

    let result = run(
        &runtime,
        &program,
        0,
        &Environment::new(),
        &[Value::Integer(1), Value::Integer(2)],
        Span::new(0, 1),
    );

    assert!(
        matches!(result, Err(RuntimeError::Arity { expected, actual, .. }) if expected == "1" && actual == 2)
    );
}

#[test]
fn accepts_extra_arguments_when_a_rest_parameter_is_declared() {
    let runtime = Runtime::new();
    let mut compiled = function(vec![
        Instruction::Load("rest".to_string()),
        Instruction::Return,
    ]);
    compiled.parameters.push("value".to_string());
    compiled.rest = Some("rest".to_string());
    let program = Rc::new(Program {
        functions: vec![compiled],
        entry: 0,
    });

    let result = match run(
        &runtime,
        &program,
        0,
        &Environment::new(),
        &[Value::Integer(1), Value::Integer(2)],
        Span::new(0, 1),
    ) {
        Ok(value) => value,
        Err(error) => panic!("a rest parameter must accept additional arguments: {error}"),
    };

    assert_eq!(result.to_string(), "(2)");
}

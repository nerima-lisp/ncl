use std::rc::Rc;

use ncl_compiler::{FunctionCode, Instruction, Program};
use ncl_syntax::Span;

use crate::vm::run;
use crate::{Environment, Runtime, RuntimeError, Value};

use super::argument_layout;

fn function(instructions: Vec<Instruction>) -> FunctionCode {
    FunctionCode {
        name: Some("test-function".to_string()),
        parameters: Vec::new(),
        required_escaped: Vec::new(),
        optional: Vec::new(),
        keywords: Vec::new(),
        has_keyword_section: false,
        allow_other_keys: false,
        rest: None,
        rest_escaped: false,
        auxiliary: Vec::new(),
        instructions,
    }
}

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

#[test]
fn argument_layout_handles_optional_keyword_and_rest_shapes() {
    let mut optional = function(vec![]);
    optional.parameters.push("required".to_string());
    optional.optional.push(ncl_compiler::OptionalParameter {
        name: "optional".to_string(),
        name_escaped: false,
        default_function: 0,
        supplied_p: None,
        supplied_p_escaped: None,
    });
    optional.has_keyword_section = true;

    let layouts = [
        (&optional, vec![Value::Integer(1)], (0, 1)),
        (
            &optional,
            vec![Value::Integer(1), Value::Keyword("key".to_string().into())],
            (0, 1),
        ),
        (
            &optional,
            vec![Value::Integer(1), Value::Integer(2)],
            (1, 2),
        ),
    ];

    for (function, arguments, expected) in layouts {
        assert!(matches!(
            argument_layout(function, &arguments),
            Ok(layout) if layout == expected
        ));
    }
}

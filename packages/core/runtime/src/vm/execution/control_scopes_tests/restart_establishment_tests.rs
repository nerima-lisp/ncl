#[allow(clippy::wildcard_imports)]
use super::*;

use ncl_compiler::{Constant, RestartBindClause};

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

#[test]
fn restart_bind_evaluates_each_binding_function_and_runs_the_body() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![constant_function(1), constant_function(2)],
        entry: 0,
    });
    let bindings = vec![RestartBindClause {
        name: "MY-RESTART".to_string(),
        function: 0,
    }];
    let mut stack = Vec::new();

    let result = execute_restart_bind_instruction(
        &runtime,
        &program,
        1,
        &bindings,
        &mut stack,
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(2)));
}

#[test]
fn restart_bind_lets_unrelated_errors_pass_through() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![
            constant_function(1),
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
                instructions: vec![Instruction::Load("missing".to_string())],
            },
        ],
        entry: 0,
    });
    let bindings = vec![RestartBindClause {
        name: "MY-RESTART".to_string(),
        function: 0,
    }];
    let mut stack = Vec::new();

    let result = execute_restart_bind_instruction(
        &runtime,
        &program,
        1,
        &bindings,
        &mut stack,
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::UnboundVariable { name, .. }) if name.eq_ignore_ascii_case("missing")
    ));
}

#[test]
fn with_simple_restart_pushes_the_bodys_value_when_the_restart_is_not_invoked() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![constant_function(11)],
        entry: 0,
    });
    let mut stack = Vec::new();

    let result = execute_with_simple_restart_instruction(
        &runtime,
        &program,
        "my-restart",
        0,
        &mut stack,
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(11)));
}

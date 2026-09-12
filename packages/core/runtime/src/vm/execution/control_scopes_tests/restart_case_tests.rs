#[allow(clippy::wildcard_imports)]
use super::*;

use ncl_compiler::{Constant, HandlerCaseClause, RestartCaseClause};

mod restart_behavior;

fn function(parameters: Vec<String>, instructions: Vec<Instruction>) -> FunctionCode {
    FunctionCode {
        name: None,
        parameters,
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

fn invoke_restart_environment() -> Environment {
    let environment = Environment::new();
    environment.define("invoke-restart", Value::primitive("INVOKE-RESTART"));
    environment
}

fn condition_producing_functions() -> Vec<FunctionCode> {
    vec![
        function(
            Vec::new(),
            vec![
                Instruction::HandlerCase {
                    protected: 1,
                    clauses: vec![HandlerCaseClause {
                        condition: "ERROR".to_string().into(),
                        variable: Some("c".to_string()),
                        function: 2,
                    }],
                },
                Instruction::Return,
            ],
        ),
        function(Vec::new(), vec![Instruction::Load("missing".to_string())]),
        function(
            vec!["c".to_string()],
            vec![Instruction::Load("c".to_string()), Instruction::Return],
        ),
    ]
}

#[test]
fn with_condition_restarts_rejects_an_out_of_range_restarts_function_id() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: condition_producing_functions(),
        entry: 0,
    });
    let mut stack = Vec::new();

    let result = execute_with_condition_restarts_instruction(
        &runtime,
        &program,
        (0, 99, 0),
        &mut stack,
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "compiled with-condition-restarts restarts function id is out of range"
    ));
}

#[test]
fn with_condition_restarts_rejects_an_out_of_range_body_function_id() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut functions = condition_producing_functions();
    functions.push(function(
        Vec::new(),
        vec![Instruction::Constant(Constant::Nil), Instruction::Return],
    ));
    let program = Rc::new(Program {
        functions,
        entry: 0,
    });
    let mut stack = Vec::new();

    let result = execute_with_condition_restarts_instruction(
        &runtime,
        &program,
        (0, 3, 99),
        &mut stack,
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "compiled with-condition-restarts body id is out of range"
    ));
}

#[test]
fn with_condition_restarts_runs_the_body_around_an_empty_restart_list() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut functions = condition_producing_functions();
    functions.push(function(
        Vec::new(),
        vec![Instruction::Constant(Constant::Nil), Instruction::Return],
    ));
    functions.push(function(
        Vec::new(),
        vec![
            Instruction::Constant(Constant::Integer(9)),
            Instruction::Return,
        ],
    ));
    let program = Rc::new(Program {
        functions,
        entry: 0,
    });
    let mut stack = Vec::new();

    let result = execute_with_condition_restarts_instruction(
        &runtime,
        &program,
        (0, 3, 4),
        &mut stack,
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(9)));
}

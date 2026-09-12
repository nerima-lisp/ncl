#[allow(clippy::wildcard_imports)]
use super::*;

#[test]
fn restart_case_pushes_the_protected_value_when_no_restart_is_invoked() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![function(
            Vec::new(),
            vec![
                Instruction::Constant(Constant::Integer(7)),
                Instruction::Return,
            ],
        )],
        entry: 0,
    });
    let mut stack = Vec::new();

    let result = execute_restart_case_instruction(
        &runtime,
        &program,
        0,
        &[],
        &mut stack,
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(7)));
}

#[test]
fn restart_case_lets_unrelated_errors_pass_through() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![
            function(Vec::new(), vec![Instruction::Load("missing".to_string())]),
            function(Vec::new(), vec![Instruction::Return]),
        ],
        entry: 0,
    });
    let clauses = vec![RestartCaseClause {
        name: "USE-VALUE".to_string(),
        function: 1,
    }];
    let mut stack = Vec::new();

    let result = execute_restart_case_instruction(
        &runtime,
        &program,
        0,
        &clauses,
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
fn restart_case_returns_the_error_when_the_invoked_restart_is_not_a_clause() {
    let runtime = Runtime::new();
    let environment = invoke_restart_environment();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![
            function(
                Vec::new(),
                vec![
                    Instruction::Load("invoke-restart".to_string()),
                    Instruction::Constant(Constant::Symbol("MISSING-RESTART".to_string())),
                    Instruction::Call(1),
                ],
            ),
            function(Vec::new(), vec![Instruction::Return]),
        ],
        entry: 0,
    });
    let clauses = vec![RestartCaseClause {
        name: "USE-VALUE".to_string(),
        function: 1,
    }];
    let mut stack = Vec::new();

    let result = execute_restart_case_instruction(
        &runtime,
        &program,
        0,
        &clauses,
        &mut stack,
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvokeRestart { name, .. }) if name == "MISSING-RESTART"
    ));
}

#[test]
fn restart_case_runs_the_matching_clause_with_the_restart_arguments() {
    let runtime = Runtime::new();
    let environment = invoke_restart_environment();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![
            function(
                Vec::new(),
                vec![
                    Instruction::Load("invoke-restart".to_string()),
                    Instruction::Constant(Constant::Symbol("USE-VALUE".to_string())),
                    Instruction::Constant(Constant::Integer(5)),
                    Instruction::Call(2),
                ],
            ),
            function(
                vec!["v".to_string()],
                vec![Instruction::Load("v".to_string()), Instruction::Return],
            ),
        ],
        entry: 0,
    });
    let clauses = vec![RestartCaseClause {
        name: "USE-VALUE".to_string(),
        function: 1,
    }];
    let mut stack = Vec::new();

    let result = execute_restart_case_instruction(
        &runtime,
        &program,
        0,
        &clauses,
        &mut stack,
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(5)));
}

use super::*;

#[test]
fn handler_case_lets_return_from_pass_through_the_protected_form() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![
            function(
                Vec::new(),
                vec![
                    Instruction::Constant(Constant::Integer(1)),
                    Instruction::ReturnFrom {
                        name: "BLOCK".to_string(),
                    },
                ],
            ),
            function(Vec::new(), vec![Instruction::Return]),
        ],
        entry: 0,
    });
    let clauses = vec![HandlerCaseClause {
        condition: "ERROR".to_string().into(),
        variable: None,
        function: 1,
    }];
    let mut stack = Vec::new();

    let result = execute_handler_case_instruction(
        &runtime,
        &program,
        0,
        &clauses,
        &mut stack,
        &environment,
        span,
    );

    assert!(matches!(result, Err(RuntimeError::ReturnFrom { .. })));
}

#[test]
fn handler_case_runs_a_matching_clause_without_a_bound_variable() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![
            function(Vec::new(), vec![Instruction::Load("missing".to_string())]),
            function(
                Vec::new(),
                vec![
                    Instruction::Constant(Constant::Integer(99)),
                    Instruction::Return,
                ],
            ),
        ],
        entry: 0,
    });
    let clauses = vec![HandlerCaseClause {
        condition: "ERROR".to_string().into(),
        variable: None,
        function: 1,
    }];
    let mut stack = Vec::new();

    let result = execute_handler_case_instruction(
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
    assert!(matches!(stack[0], Value::Integer(99)));
}

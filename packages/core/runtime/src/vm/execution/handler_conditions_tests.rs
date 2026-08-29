#[allow(clippy::wildcard_imports)]
use super::*;

use ncl_compiler::{Constant, HandlerBindClause, HandlerCaseClause};

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
        condition: "ERROR".to_string(),
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
        condition: "ERROR".to_string(),
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

#[test]
fn handler_bind_returns_the_error_when_no_handler_matches() {
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
    let handlers = vec![HandlerBindClause {
        condition: "PACKAGE-ERROR".to_string(),
        function: 1,
    }];
    let mut stack = Vec::new();

    let result = execute_handler_bind_instruction(
        &runtime,
        &program,
        0,
        &handlers,
        &mut stack,
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::UnboundVariable { name, .. }) if name.eq_ignore_ascii_case("missing")
    ));
    assert!(stack.is_empty());
}

#[test]
fn handler_bind_runs_a_matching_handler_with_the_signaled_condition() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![
            function(Vec::new(), vec![Instruction::Load("missing".to_string())]),
            function(
                vec!["c".to_string()],
                vec![Instruction::Load("c".to_string()), Instruction::Return],
            ),
        ],
        entry: 0,
    });
    let handlers = vec![HandlerBindClause {
        condition: "UNBOUND-VARIABLE".to_string(),
        function: 1,
    }];
    let mut stack = Vec::new();

    let result = execute_handler_bind_instruction(
        &runtime,
        &program,
        0,
        &handlers,
        &mut stack,
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Condition(_)));
}

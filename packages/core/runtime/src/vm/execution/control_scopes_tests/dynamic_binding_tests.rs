#[allow(clippy::wildcard_imports)]
use super::*;

use ncl_syntax::Form;

fn function(instructions: Vec<Instruction>) -> FunctionCode {
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
        instructions,
    }
}

#[test]
fn progv_binds_symbols_dynamically_around_the_body() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![
            function(vec![
                Instruction::Quote(Form::list(vec![Form::atom("x", span)], span)),
                Instruction::Return,
            ]),
            function(vec![
                Instruction::Quote(Form::list(vec![Form::atom("1", span)], span)),
                Instruction::Return,
            ]),
            function(vec![
                Instruction::Load("x".to_string()),
                Instruction::Return,
            ]),
        ],
        entry: 0,
    });
    let mut stack = Vec::new();

    let result = execute_progv_instruction(
        &runtime,
        &program,
        (0, 1, 2),
        &mut stack,
        &environment,
        span,
    );

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(1)));
    assert!(
        environment.lookup("x").is_none(),
        "progv bindings must not leak into the lexical environment"
    );
}

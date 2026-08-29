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

#[test]
fn catch_pushes_the_bodys_value_when_nothing_is_thrown() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![constant_function(1), constant_function(42)],
        entry: 0,
    });
    let mut stack = Vec::new();

    let result =
        execute_catch_instruction(&runtime, &program, 0, 1, &mut stack, &environment, span);

    assert!(result.is_ok());
    assert_eq!(stack.len(), 1);
    assert!(matches!(stack[0], Value::Integer(42)));
}

#[test]
fn unwind_protect_rejects_an_out_of_range_cleanup_function_id() {
    let runtime = Runtime::new();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let program = Rc::new(Program {
        functions: vec![constant_function(1)],
        entry: 0,
    });
    let mut stack = Vec::new();

    let result = execute_unwind_protect_instruction(
        &runtime,
        &program,
        (0, 1),
        &mut stack,
        &environment,
        span,
    );

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "compiled unwind-protect cleanup function id is out of range"
    ));
    assert!(stack.is_empty());
}

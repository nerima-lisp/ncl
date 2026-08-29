#[allow(clippy::wildcard_imports)]
use super::*;

fn empty_program() -> Rc<Program> {
    Rc::new(Program {
        functions: Vec::new(),
        entry: 0,
    })
}

fn assert_invalid(result: Result<(), RuntimeError>, expected: &str) {
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. }) if message == expected
    ));
}

#[test]
fn scope_instructions_reject_out_of_range_function_ids() {
    let runtime = Runtime::new();
    let program = empty_program();
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut stack = Vec::new();

    assert_invalid(
        execute_restart_bind_instruction(
            &runtime,
            &program,
            0,
            &[],
            &mut stack,
            &environment,
            span,
        ),
        "compiled restart-bind body id is out of range",
    );
    assert_invalid(
        execute_catch_instruction(&runtime, &program, 0, 0, &mut stack, &environment, span),
        "compiled catch tag function id is out of range",
    );
    assert_invalid(
        execute_with_simple_restart_instruction(
            &runtime,
            &program,
            "restart",
            0,
            &mut stack,
            &environment,
            span,
        ),
        "compiled with-simple-restart body id is out of range",
    );
    assert_invalid(
        execute_restart_case_instruction(
            &runtime,
            &program,
            0,
            &[],
            &mut stack,
            &environment,
            span,
        ),
        "compiled restart-case protected function id is out of range",
    );
    assert_invalid(
        execute_with_condition_restarts_instruction(
            &runtime,
            &program,
            (0, 0, 0),
            &mut stack,
            &environment,
            span,
        ),
        "compiled with-condition-restarts condition function id is out of range",
    );
    assert_invalid(
        execute_progv_instruction(
            &runtime,
            &program,
            (0, 0, 0),
            &mut stack,
            &environment,
            span,
        ),
        "compiled progv symbol function id is out of range",
    );
    assert_invalid(
        execute_block_instruction(
            &runtime,
            &program,
            0,
            "block",
            &mut stack,
            &environment,
            span,
        ),
        "compiled block function id is out of range",
    );
    assert_invalid(
        execute_tagbody_instruction(&runtime, &program, 0, &[], &mut stack, &environment, span),
        "compiled tagbody function id is out of range",
    );
    assert_invalid(
        execute_unwind_protect_instruction(
            &runtime,
            &program,
            (0, 0),
            &mut stack,
            &environment,
            span,
        ),
        "compiled unwind-protect protected function id is out of range",
    );
}

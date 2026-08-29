#[allow(clippy::wildcard_imports)]
use super::*;

fn assert_invalid(result: Result<(), RuntimeError>, expected: &str) {
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. }) if message == expected
    ));
}

#[test]
fn closure_instructions_reject_out_of_range_function_ids() {
    let runtime = Runtime::new();
    let function = FunctionCode {
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
        instructions: vec![Instruction::Return],
    };
    let program = Rc::new(Program {
        functions: vec![function.clone()],
        entry: 0,
    });
    let environment = Environment::new();
    let span = Span::new(0, 1);
    let mut stack = Vec::new();
    let mut program_counter = 0;
    let mut branch_context = BranchInstructionContext {
        runtime: &runtime,
        program: &program,
        function: &function,
        stack: &mut stack,
        environment: &environment,
        program_counter: &mut program_counter,
        span,
    };
    for (instruction, expected) in [
        (
            Instruction::MakeClosure(1),
            "compiled closure id is out of range",
        ),
        (
            Instruction::IgnoreErrors(1),
            "compiled ignore-errors function id is out of range",
        ),
    ] {
        assert_invalid(
            execute_binding_and_branch_instruction(&instruction, &mut branch_context).map(|_| ()),
            expected,
        );
    }
}

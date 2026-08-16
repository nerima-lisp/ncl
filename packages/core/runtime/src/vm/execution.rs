
struct ExecutionState<'a> {
    runtime: &'a Runtime,
    program: &'a Rc<Program>,
    function: &'a FunctionCode,
    environment: &'a mut Environment,
    stack: &'a mut Vec<Value>,
    scopes: &'a mut Vec<(Environment, usize, usize)>,
    span: Span,
    program_counter: &'a mut usize,
}

enum ExecutionOutcome {
    Continue,
    Return(Value),
}

include!("execution/state.rs");
include!("execution/conditions.rs");
include!("execution/dynamic.rs");
include!("execution/calls.rs");

fn execute_instruction(
    instruction: &Instruction,
    state: &mut ExecutionState<'_>,
) -> Result<ExecutionOutcome, RuntimeError> {
    if let Some(outcome) = execute_state_instruction(instruction, state)? {
        return Ok(outcome);
    }
    if let Some(outcome) = execute_condition_instruction(instruction, state)? {
        return Ok(outcome);
    }
    if let Some(outcome) = execute_dynamic_instruction(instruction, state)? {
        return Ok(outcome);
    }
    if let Some(outcome) = execute_call_instruction(instruction, state)? {
        return Ok(outcome);
    }
    Err(invalid(
        "compiled function contains an unsupported instruction",
        state.span,
    ))
}

fn run_code(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    environment: Environment,
    span: Span,
) -> Result<Value, RuntimeError> {
    run_code_from(runtime, program, function, environment, span, 0)
}

fn run_code_from(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    mut environment: Environment,
    span: Span,
    start_program_counter: usize,
) -> Result<Value, RuntimeError> {
    let mut stack = Vec::new();
    let mut scopes: Vec<(Environment, usize, usize)> = Vec::new();
    let _dynamic_guard = runtime.dynamic_guard();
    let mut program_counter = start_program_counter;

    let mut state = ExecutionState {
        runtime,
        program,
        function,
        environment: &mut environment,
        stack: &mut stack,
        scopes: &mut scopes,
        span,
        program_counter: &mut program_counter,
    };

    loop {
        let Some(instruction) = state
            .function
            .instructions
            .get(*state.program_counter)
            .cloned()
        else {
            return Err(invalid(
                "compiled function reached an invalid instruction pointer",
                span,
            ));
        };

        match execute_instruction(&instruction, &mut state)? {
            ExecutionOutcome::Continue => {}
            ExecutionOutcome::Return(value) => return Ok(value),
        }
    }
}

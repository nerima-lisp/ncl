#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) struct PreControlInstructionContext<'a> {
    pub(super) runtime: &'a Runtime,
    pub(super) program: &'a Rc<Program>,
    pub(super) function: &'a FunctionCode,
    pub(super) stack: &'a mut Vec<Value>,
    pub(super) scopes: &'a mut Vec<(Environment, usize, usize)>,
    pub(super) environment: &'a mut Environment,
    pub(super) program_counter: &'a mut usize,
    pub(super) span: Span,
}

pub(super) fn execute_pre_control_instruction(
    instruction: &Instruction,
    context: &mut PreControlInstructionContext<'_>,
) -> Result<bool, RuntimeError> {
    if execute_load_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.environment,
        context.program_counter,
        context.span,
    )? || execute_definition_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.environment,
        context.program_counter,
        context.span,
    )? || execute_set_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.environment,
        context.program_counter,
        context.span,
    )? || execute_parallel_set_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.environment,
        context.program_counter,
        context.span,
    )? {
        return Ok(true);
    }
    if execute_stack_instruction(
        context.runtime,
        instruction,
        context.stack,
        context.scopes,
        context.environment,
        context.program_counter,
        context.span,
    )? {
        return Ok(true);
    }
    let mut branch_context = BranchInstructionContext {
        runtime: context.runtime,
        program: context.program,
        function: context.function,
        stack: context.stack,
        environment: &*context.environment,
        program_counter: context.program_counter,
        span: context.span,
    };
    execute_binding_and_branch_instruction(instruction, &mut branch_context)
}

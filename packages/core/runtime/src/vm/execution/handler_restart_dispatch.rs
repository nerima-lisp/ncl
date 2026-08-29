#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn execute_handler_restart_instruction(
    instruction: &Instruction,
    context: &mut ControlInstructionContext<'_>,
) -> Result<(), RuntimeError> {
    match instruction {
        Instruction::HandlerCase { protected, clauses } => execute_handler_case_instruction(
            context.runtime,
            context.program,
            *protected,
            clauses,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::HandlerBind { body, handlers } => execute_handler_bind_instruction(
            context.runtime,
            context.program,
            *body,
            handlers,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::RestartBind { body, bindings } => execute_restart_bind_instruction(
            context.runtime,
            context.program,
            *body,
            bindings,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::Catch { tag, body } => execute_catch_instruction(
            context.runtime,
            context.program,
            *tag,
            *body,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::WithSimpleRestart { name, body } => execute_with_simple_restart_instruction(
            context.runtime,
            context.program,
            name,
            *body,
            context.stack,
            context.environment,
            context.span,
        )?,
        _ => unreachable!("handler/restart instruction was not dispatched"),
    }
    *context.program_counter += 1;
    Ok(())
}

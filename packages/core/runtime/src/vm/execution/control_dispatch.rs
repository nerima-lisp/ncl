#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) struct ControlInstructionContext<'a> {
    pub(super) runtime: &'a Runtime,
    pub(super) program: &'a Rc<Program>,
    pub(super) stack: &'a mut Vec<Value>,
    pub(super) environment: &'a Environment,
    pub(super) program_counter: &'a mut usize,
    pub(super) span: Span,
}

pub(super) fn execute_control_instruction(
    instruction: &Instruction,
    context: &mut ControlInstructionContext<'_>,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::HandlerCase { .. }
        | Instruction::HandlerBind { .. }
        | Instruction::RestartBind { .. }
        | Instruction::Catch { .. }
        | Instruction::WithSimpleRestart { .. } => {
            execute_handler_restart_instruction(instruction, context)?;
            Ok(true)
        }
        _ => execute_scope_control_instruction(instruction, context),
    }
}

pub(super) fn execute_scope_control_instruction(
    instruction: &Instruction,
    context: &mut ControlInstructionContext<'_>,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::RestartCase { protected, clauses } => execute_restart_case_instruction(
            context.runtime,
            context.program,
            *protected,
            clauses,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::WithConditionRestarts {
            condition,
            restarts,
            body,
        } => execute_with_condition_restarts_instruction(
            context.runtime,
            context.program,
            (*condition, *restarts, *body),
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::Progv {
            symbols,
            values,
            body,
        } => execute_progv_instruction(
            context.runtime,
            context.program,
            (*symbols, *values, *body),
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::Throw => {
            let value = pop_value(context.stack, context.span, "throw")?;
            let tag = pop_value(context.stack, context.span, "throw")?.primary_value();
            return Err(RuntimeError::Throw {
                tag: ThrowTag::new(tag),
                value: ReturnValue::new(value),
                span: Some(context.span),
            });
        }
        Instruction::Block {
            function: function_id,
            name,
        } => execute_block_instruction(
            context.runtime,
            context.program,
            *function_id,
            name,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::TagBody {
            function: function_id,
            tags,
        } => execute_tagbody_instruction(
            context.runtime,
            context.program,
            *function_id,
            tags,
            context.stack,
            context.environment,
            context.span,
        )?,
        Instruction::Go { tag } => {
            return Err(RuntimeError::Go {
                tag: tag.clone(),
                target: context.environment.lookup_tag(tag),
                span: Some(context.span),
            });
        }
        Instruction::UnwindProtect {
            protected: protected_id,
            cleanup: cleanup_id,
        } => execute_unwind_protect_instruction(
            context.runtime,
            context.program,
            (*protected_id, *cleanup_id),
            context.stack,
            context.environment,
            context.span,
        )?,
        _ => return Ok(false),
    }
    *context.program_counter += 1;
    Ok(true)
}

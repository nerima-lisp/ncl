#[allow(clippy::wildcard_imports)]
use super::*;
use ncl_syntax::FormKind;

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
        Instruction::SetfPlaces { invocation, values } => {
            let FormKind::List(items) = &invocation.kind else {
                return Err(invalid("invalid SETF invocation", context.span));
            };
            if items.len().is_multiple_of(2) || items.len() / 2 != values.len() {
                return Err(invalid("invalid SETF value functions", context.span));
            }
            let value = context.runtime.execute_sequential_assignment(
                items,
                context.environment,
                |index| {
                    let code = context
                        .program
                        .functions
                        .get(values[index])
                        .ok_or_else(|| invalid("invalid SETF value function", context.span))?;
                    crate::vm::entry::run_code(
                        context.runtime,
                        context.program,
                        code,
                        context.environment.clone(),
                        context.span,
                    )
                },
            )?;
            context.stack.push(value);
            *context.program_counter += 1;
            Ok(true)
        }
        Instruction::ModifyPlace {
            invocation,
            delta,
            arithmetic,
        } => {
            let code = context
                .program
                .functions
                .get(*delta)
                .ok_or_else(|| invalid("invalid modifying delta function", context.span))?;
            let value = context.runtime.execute_compiled_modify_place(
                invocation,
                arithmetic,
                context.environment,
                || {
                    crate::vm::entry::run_code(
                        context.runtime,
                        context.program,
                        code,
                        context.environment.clone(),
                        context.span,
                    )
                },
            )?;
            context.stack.push(value);
            *context.program_counter += 1;
            Ok(true)
        }
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
        Instruction::StandardStreamBind {
            input,
            stream,
            variable,
            index,
            destination,
            body,
        } => execute_standard_stream_bind_instruction(
            context.runtime,
            context.program,
            *input,
            *stream,
            variable,
            index.as_deref(),
            destination.as_deref(),
            *body,
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

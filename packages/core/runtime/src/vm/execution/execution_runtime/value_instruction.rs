use ncl_compiler::Instruction;
use ncl_syntax::Span;

use crate::vm::execution::application::{
    execute_apply_instruction, execute_call_instruction, execute_list_mapping_instruction,
    execute_multiple_value_call_instruction,
};
use crate::vm::primitives::pop_value;
use crate::{Environment, Runtime, RuntimeError, Value};

pub(super) fn execute_value_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Eval(form_span) => {
            let value = pop_value(stack, span, "eval")?.primary_value();
            let form = Runtime::form_from_value(&value, *form_span)?;
            stack.push(runtime.eval_values_in(&form, environment)?);
        }
        Instruction::Call(argument_count) => {
            execute_call_instruction(runtime, *argument_count, stack, environment, span)?;
        }
        Instruction::Apply(argument_count) => {
            execute_apply_instruction(runtime, *argument_count, stack, environment, span)?;
        }
        Instruction::ListMapping {
            operation,
            sequence_count,
        } => {
            execute_list_mapping_instruction(
                runtime,
                operation,
                *sequence_count,
                stack,
                environment,
                span,
            )?;
        }
        Instruction::MultipleValueCall(value_form_count) => {
            execute_multiple_value_call_instruction(
                runtime,
                *value_form_count,
                stack,
                environment,
                span,
            )?;
        }
        _ => return Ok(false),
    }
    *program_counter += 1;
    Ok(true)
}

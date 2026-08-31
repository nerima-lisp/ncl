#[allow(clippy::wildcard_imports)]
use super::*;

mod array;
mod list;
mod property;
mod sequence;
mod symbol_cell;

pub(super) fn execute_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if sequence::execute(
        runtime,
        instruction,
        stack,
        environment,
        program_counter,
        span,
    )? {
        return Ok(true);
    }
    if property::execute(
        runtime,
        instruction,
        stack,
        environment,
        program_counter,
        span,
    )? {
        return Ok(true);
    }
    if array::execute(
        runtime,
        instruction,
        stack,
        environment,
        program_counter,
        span,
    )? {
        return Ok(true);
    }
    match instruction {
        Instruction::SetfSymbolCellDynamic { operator } => {
            symbol_cell::execute(runtime, operator, stack, program_counter, span)
        }
        Instruction::Set(name) | Instruction::SetExact(name) => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("setq has no value on the stack", span))?
                .primary_value();
            if matches!(instruction, Instruction::Set(_)) {
                runtime.set_or_define_in(name, value.clone(), environment, span)?;
            } else {
                runtime.set_or_define_exact_in(name, value.clone(), environment, span)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfList {
            operator,
            name,
            escaped,
        } => list::execute(
            runtime,
            operator,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfElementDynamic {
            operator,
            name,
            escaped,
        } => sequence::execute_element(
            runtime,
            operator,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfSubseqDynamic {
            has_end,
            name,
            escaped,
        } => sequence::execute_subseq(
            runtime,
            *has_end,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushNewList { name, escaped } => list::execute_pushnew(
            runtime,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushNewListOptions {
            name,
            escaped,
            test_not,
            has_key,
            key_before_test,
        } => list::execute_pushnew_options(
            runtime,
            name,
            *escaped,
            *test_not,
            *has_key,
            *key_before_test,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::MapIntoSetfSymbol { name, escaped } => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("map-into has no value on the stack", span))?
                .primary_value();
            if *escaped {
                runtime.set_or_define_exact_in(name, value.clone(), environment, span)?;
            } else {
                runtime.set_or_define_in(name, value.clone(), environment, span)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("map-into has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::Setf(place) | Instruction::MapIntoSetf(place) => {
            let map_into = matches!(instruction, Instruction::MapIntoSetf(_));
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| {
                    invalid(
                        if map_into {
                            "map-into has no value on the stack"
                        } else {
                            "setf has no value on the stack"
                        },
                        span,
                    )
                })?
                .primary_value();
            if map_into {
                runtime.set_map_into_destination(place, value.clone(), environment)?;
            } else {
                runtime.set_place(place, value.clone(), environment)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("setf has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

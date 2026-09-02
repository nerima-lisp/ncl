#[allow(clippy::wildcard_imports)]
use super::*;

mod array;
mod bitfield;
mod element;
pub(super) mod list;
mod property;
mod pushnew;
mod pushnew_nested;
mod rotate_shift;
mod sequence;
mod subseq;
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
    if bitfield::execute(
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
        Instruction::SetfNestedList {
            accessors,
            name,
            escaped,
        } => list::execute_nested(
            runtime,
            accessors,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfNestedNthDynamic { accessors, name, escaped } => list::execute_nested_nth_dynamic(
            runtime, accessors, name, *escaped, stack, environment, program_counter, span,
        ),
        Instruction::ListPlaceMutation {
            operator,
            accessor,
            name,
            escaped,
        } => list::execute_place_mutation(
            runtime,
            operator,
            accessor,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::NestedListPlaceMutation {
            accessors,
            operator,
            name,
            escaped,
        } => list::execute_nested_place_mutation(
            runtime,
            accessors,
            operator,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::NestedListMutationNthDynamic { .. } => sequence::execute(
            runtime,
            instruction,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::NestedListMutationNthPushNewOptions { .. } => sequence::execute(
            runtime,
            instruction,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::ListPlacePushNewOptions {
            accessor,
            name,
            escaped,
            test_not,
            has_key,
            key_before_test,
        } => pushnew::execute_place_pushnew_options(
            runtime,
            accessor,
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
        Instruction::NestedListPlacePushNewOptions {
            accessors,
            name,
            escaped,
            test_not,
            has_key,
            key_before_test,
        } => pushnew_nested::execute_nested_place_pushnew_options(
            runtime,
            accessors,
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
        Instruction::SetfElementDynamic {
            operator,
            name,
            escaped,
        } => element::execute(
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
        } => subseq::execute(
            runtime,
            *has_end,
            name,
            *escaped,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushNewList { name, escaped } => pushnew::execute_pushnew(
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
        } => pushnew::execute_pushnew_options(
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

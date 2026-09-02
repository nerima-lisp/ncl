use ncl_compiler::Instruction;
use ncl_syntax::Span;

use crate::vm::execution::application::{
    execute_array_adjustment_instruction, execute_array_construction_instruction,
    execute_array_element_instruction, execute_array_metadata_instruction,
    execute_character_comparison_instruction, execute_character_digit_instruction,
    execute_character_digit_predicate_instruction, execute_character_element_instruction,
    execute_list_construction_with_options_instruction, execute_multiple_value_call_instruction,
    execute_string_case_instruction, execute_string_comparison_instruction,
    execute_string_construction_instruction, execute_string_trim_instruction,
};
use crate::{Environment, Runtime, RuntimeError, Value};

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<Option<()>, RuntimeError> {
    match instruction {
        Instruction::ArrayConstruction { argument_count } => {
            execute_array_construction_instruction(stack, *argument_count, span)?
        }
        Instruction::ArrayAdjustment { argument_count } => {
            execute_array_adjustment_instruction(stack, *argument_count, span)?
        }
        Instruction::ListConstructionWithOptions { argument_count } => {
            execute_list_construction_with_options_instruction(stack, *argument_count, span)?
        }
        Instruction::StringCase {
            operation,
            argument_count,
        } => execute_string_case_instruction(stack, operation, *argument_count, span)?,
        Instruction::StringComparison { operation } => {
            execute_string_comparison_instruction(stack, operation, span)?
        }
        Instruction::StringTrim { operation } => {
            execute_string_trim_instruction(stack, operation, span)?
        }
        Instruction::StringConstruction {
            operation,
            argument_count,
        } => execute_string_construction_instruction(stack, operation, *argument_count, span)?,
        Instruction::CharacterComparison {
            operation,
            argument_count,
        } => execute_character_comparison_instruction(stack, operation, *argument_count, span)?,
        Instruction::CharacterElement { operation } => {
            execute_character_element_instruction(stack, operation, span)?
        }
        Instruction::CharacterDigitPredicate { argument_count } => {
            execute_character_digit_predicate_instruction(stack, *argument_count, span)?
        }
        Instruction::CharacterDigit { argument_count } => {
            execute_character_digit_instruction(stack, *argument_count, span)?
        }
        Instruction::ArrayElement {
            operation,
            argument_count,
        } => execute_array_element_instruction(stack, operation, *argument_count, span)?,
        Instruction::ArrayMetadata {
            operation,
            argument_count,
        } => execute_array_metadata_instruction(stack, operation, *argument_count, span)?,
        Instruction::MultipleValueCall(value_form_count) => {
            execute_multiple_value_call_instruction(
                runtime,
                *value_form_count,
                stack,
                environment,
                span,
            )?
        }
        _ => return Ok(None),
    }
    Ok(Some(()))
}

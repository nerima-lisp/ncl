use ncl_compiler::Instruction;
use ncl_syntax::{FormKind, Span};

use super::mutation_instruction;
use crate::vm::execution::application::{
    execute_apply_instruction, execute_association_search_instruction, execute_call_instruction,
    execute_list_membership_instruction,
    execute_list_mapping_instruction,
    execute_multiple_value_call_instruction, execute_sequence_mapping_instruction,
    execute_sequence_map_into_instruction,
    execute_sequence_merge_instruction, execute_sequence_reduce_instruction,
    execute_sequence_pair_search_instruction, execute_sequence_removal_instruction,
    execute_sequence_search_instruction,
    execute_sequence_sort_instruction,
    execute_sequence_quantifier_instruction,
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
        Instruction::Defstruct(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFSTRUCT instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(runtime.special_defstruct(items, environment)?);
        }
        Instruction::Defclass(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFCLASS instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(Runtime::special_defclass(items, environment)?);
        }
        Instruction::Defgeneric(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFGENERIC instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(Runtime::special_defgeneric(items, environment)?);
        }
        Instruction::Defmethod(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFMETHOD instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(Runtime::special_defmethod(items, environment)?);
        }
        Instruction::Defsetf(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFSETF instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(runtime.special_defsetf(items, environment)?);
        }
        Instruction::Defconstant(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFCONSTANT instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(runtime.special_defconstant(items, environment)?);
        }
        Instruction::DefineSymbolMacro(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFINE-SYMBOL-MACRO instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(Runtime::special_define_symbol_macro(items, environment)?);
        }
        Instruction::DefineModifyMacro(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFINE-MODIFY-MACRO instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(runtime.special_define_modify_macro(items, environment)?);
        }
        Instruction::DefineSetfExpander(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "DEFINE-SETF-EXPANDER instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(Runtime::special_define_setf_expander(items, environment)?);
        }
        Instruction::GetSetfExpansion(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "GET-SETF-EXPANSION instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(runtime.special_get_setf_expansion(items, environment)?);
        }
        Instruction::Psetf(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "PSETF instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(runtime.special_psetf(items, environment)?);
        }
        Instruction::LoadTimeValue(form) => {
            let FormKind::List(items) = &form.kind else {
                return Err(RuntimeError::InvalidForm {
                    message: "LOAD-TIME-VALUE instruction requires a list".to_string(),
                    span: Some(form.span),
                });
            };
            stack.push(runtime.special_load_time_value(items, environment)?);
        }
        Instruction::RuntimeMutation(form) => {
            mutation_instruction::execute(runtime, form, stack, environment)?;
        }
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
        Instruction::SequenceQuantifier {
            operation,
            sequence_count,
        } => {
            execute_sequence_quantifier_instruction(
                runtime,
                operation,
                *sequence_count,
                stack,
                environment,
                span,
            )?;
        }
        Instruction::SequenceMapping { sequence_count } => {
            execute_sequence_mapping_instruction(runtime, *sequence_count, stack, environment, span)?;
        }
        Instruction::SequenceMapInto { sequence_count } => {
            execute_sequence_map_into_instruction(runtime, *sequence_count, stack, environment, span)?;
        }
        Instruction::SequenceReduce { option_count } => {
            execute_sequence_reduce_instruction(runtime, *option_count, stack, environment, span)?;
        }
        Instruction::SequenceMerge { option_count } => {
            execute_sequence_merge_instruction(runtime, *option_count, stack, environment, span)?;
        }
        Instruction::SequenceSort { operation, option_count } => {
            execute_sequence_sort_instruction(runtime, operation, *option_count, stack, environment, span)?;
        }
        Instruction::SequenceSearch { operation, predicate, option_count } => {
            execute_sequence_search_instruction(runtime, *predicate, operation, *option_count, stack, environment, span)?;
        }
        Instruction::SequencePairSearch { operation, option_count } => {
            execute_sequence_pair_search_instruction(runtime, operation, *option_count, stack, environment, span)?;
        }
        Instruction::ListMembership { operation, predicate, option_count } => {
            execute_list_membership_instruction(runtime, operation, *predicate, *option_count, stack, environment, span)?;
        }
        Instruction::AssociationSearch { operation, predicate, option_count } => {
            execute_association_search_instruction(runtime, operation, *predicate, *option_count, stack, environment, span)?;
        }
        Instruction::SequenceRemoval { operation, predicate, duplicates, option_count } => {
            execute_sequence_removal_instruction(runtime, operation, *predicate, *duplicates, *option_count, stack, environment, span)?;
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

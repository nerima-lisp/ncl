use ncl_compiler::Instruction;
use ncl_syntax::{FormKind, Span};

use super::mutation_instruction;
use crate::vm::execution::application::{
    execute_apply_instruction, execute_association_search_instruction, execute_call_instruction,
    execute_list_membership_instruction,
    execute_list_construction_instruction, execute_list_construction_with_options_instruction,
    execute_list_append_instruction,
    execute_class_introspection_instruction, execute_slot_operation_instruction, execute_condition_operation_instruction, execute_restart_operation_instruction, execute_method_operation_instruction, execute_evaluation_operation_instruction, execute_package_introspection_instruction, execute_package_listing_instruction, execute_package_mutation_instruction, execute_property_list_instruction, execute_symbol_binding_instruction, execute_symbol_creation_instruction, execute_symbol_function_instruction, execute_symbol_value_instruction,
    execute_hash_table_instruction,
    execute_list_binary_instruction, execute_list_tail_instruction, execute_list_unary_instruction,
    execute_character_unary_instruction, execute_symbol_unary_instruction, execute_value_unary_instruction, execute_equality_instruction,
    execute_type_predicate_instruction,
    execute_numeric_unary_instruction,
    execute_numeric_rounding_instruction,
    execute_numeric_comparison_instruction,
    execute_numeric_fold_instruction,
    execute_numeric_binary_instruction,
    execute_numeric_boole_instruction,
    execute_numeric_bitfield_instruction,
    execute_numeric_float_instruction,
    execute_character_digit_predicate_instruction,
    execute_list_mapping_instruction,
    execute_list_set_instruction,
    execute_multiple_value_call_instruction, execute_sequence_mapping_instruction,
    execute_sequence_map_into_instruction,
    execute_sequence_merge_instruction, execute_sequence_reduce_instruction,
    execute_sequence_pair_search_instruction, execute_sequence_removal_instruction,
    execute_sequence_substitution_instruction,
    execute_sequence_unary_instruction,
    execute_sequence_length_instruction,
    execute_sequence_element_instruction,
    execute_sequence_subseq_instruction,
    execute_sequence_mutation_instruction,
    execute_sequence_concatenate_instruction,
    execute_sequence_conversion_instruction,
    execute_vector_construction_instruction,
    execute_array_construction_instruction, execute_array_adjustment_instruction,
    execute_string_case_instruction,
    execute_string_comparison_instruction,
    execute_string_trim_instruction,
    execute_string_construction_instruction,
    execute_character_comparison_instruction,
    execute_character_element_instruction,
    execute_array_element_instruction,
    execute_array_metadata_instruction,
    execute_tree_equal_instruction,
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
        Instruction::SequenceSubstitution { operation, predicate, option_count } => {
            execute_sequence_substitution_instruction(runtime, operation, *predicate, *option_count, stack, environment, span)?;
        }
        Instruction::SequenceUnary { operation } => {
            execute_sequence_unary_instruction(runtime, operation, stack, environment, span)?;
        }
        Instruction::ListUnary { operation } => {
            execute_list_unary_instruction(runtime, operation, stack, environment, span)?;
        }
        Instruction::CharacterUnary { operation } => {
            execute_character_unary_instruction(operation, stack, span)?;
        }
        Instruction::SymbolUnary { operation } => {
            execute_symbol_unary_instruction(operation, stack, span)?;
        }
        Instruction::ValueUnary { operation } => {
            execute_value_unary_instruction(operation, stack, span)?;
        }
        Instruction::TypePredicate { operation } => {
            execute_type_predicate_instruction(operation, stack, span)?;
        }
        Instruction::Equality { operation } => {
            execute_equality_instruction(operation, stack, span)?;
        }
        Instruction::NumericUnary { operation } => {
            execute_numeric_unary_instruction(operation, stack, span)?;
        }
        Instruction::NumericRounding { operation, argument_count } => {
            execute_numeric_rounding_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::NumericComparison { operation, argument_count } => {
            execute_numeric_comparison_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::NumericFold { operation, argument_count } => {
            execute_numeric_fold_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::NumericBinary { operation } => {
            execute_numeric_binary_instruction(stack, operation, span)?;
        }
        Instruction::NumericBoole => {
            execute_numeric_boole_instruction(stack, span)?;
        }
        Instruction::NumericBitfield { operation, argument_count } => {
            execute_numeric_bitfield_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::NumericFloat { operation, argument_count } => {
            execute_numeric_float_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::ListTail { operation, option_count } => {
            execute_list_tail_instruction(operation, *option_count, stack, span)?;
        }
        Instruction::ListBinary { operation } => {
            execute_list_binary_instruction(operation, stack, span)?;
        }
        Instruction::TreeEqual { option_count } => {
            execute_tree_equal_instruction(runtime, *option_count, stack, environment, span)?;
        }
        Instruction::ListSet { operation, option_count } => {
            execute_list_set_instruction(runtime, operation, *option_count, stack, environment, span)?;
        }
        Instruction::SequenceLength => {
            execute_sequence_length_instruction(stack, span)?;
        }
        Instruction::SequenceElement => {
            execute_sequence_element_instruction(stack, span)?;
        }
        Instruction::SequenceSubseq { argument_count } => {
            execute_sequence_subseq_instruction(stack, *argument_count, span)?;
        }
        Instruction::SequenceMutation { operation, argument_count } => {
            execute_sequence_mutation_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::SequenceConcatenate { argument_count } => {
            execute_sequence_concatenate_instruction(stack, *argument_count, span)?;
        }
        Instruction::SequenceConversion {
            operation,
            argument_count,
        } => {
            execute_sequence_conversion_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::VectorConstruction { argument_count } => {
            execute_vector_construction_instruction(stack, *argument_count, span)?;
        }
        Instruction::ListConstruction { argument_count, dotted } => {
            execute_list_construction_instruction(stack, *argument_count, *dotted, span)?;
        }
        Instruction::ListAppend { operation, argument_count } => {
            execute_list_append_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::PropertyList { operation, argument_count } => {
            execute_property_list_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::SymbolValue { operation, argument_count } => {
            execute_symbol_value_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::SymbolBinding { operation, argument_count } => {
            execute_symbol_binding_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::SymbolFunction { operation, argument_count } => {
            execute_symbol_function_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::SymbolCreation { operation, argument_count } => {
            execute_symbol_creation_instruction(runtime, stack, operation, *argument_count, span)?;
        }
        Instruction::ClassIntrospection { operation, argument_count } => {
            execute_class_introspection_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::SlotOperation { operation, argument_count } => {
            execute_slot_operation_instruction(runtime, stack, operation, *argument_count, span)?;
        }
        Instruction::ConditionOperation { operation, argument_count } => {
            execute_condition_operation_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::RestartOperation { operation, argument_count } => {
            execute_restart_operation_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::MethodOperation { operation, argument_count } => {
            execute_method_operation_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::EvaluationOperation { operation, argument_count } => {
            execute_evaluation_operation_instruction(runtime, stack, environment, operation, *argument_count, span)?;
        }
        Instruction::PackageIntrospection { operation, argument_count } => {
            execute_package_introspection_instruction(runtime, stack, operation, *argument_count, span)?;
        }
        Instruction::PackageMutation { operation, argument_count } => {
            execute_package_mutation_instruction(runtime, stack, operation, *argument_count, span)?;
        }
        Instruction::PackageListing { operation, argument_count } => {
            execute_package_listing_instruction(runtime, stack, operation, *argument_count, span)?;
        }
        Instruction::HashTable { operation, argument_count } => {
            execute_hash_table_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::ArrayConstruction { argument_count } => {
            execute_array_construction_instruction(stack, *argument_count, span)?;
        }
        Instruction::ArrayAdjustment { argument_count } => {
            execute_array_adjustment_instruction(stack, *argument_count, span)?;
        }
        Instruction::ListConstructionWithOptions { argument_count } => {
            execute_list_construction_with_options_instruction(stack, *argument_count, span)?;
        }
        Instruction::StringCase { operation, argument_count } => {
            execute_string_case_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::StringComparison { operation } => {
            execute_string_comparison_instruction(stack, operation, span)?;
        }
        Instruction::StringTrim { operation } => {
            execute_string_trim_instruction(stack, operation, span)?;
        }
        Instruction::StringConstruction { operation, argument_count } => {
            execute_string_construction_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::CharacterComparison { operation, argument_count } => {
            execute_character_comparison_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::CharacterElement { operation } => {
            execute_character_element_instruction(stack, operation, span)?;
        }
        Instruction::CharacterDigitPredicate { argument_count } => {
            execute_character_digit_predicate_instruction(stack, *argument_count, span)?;
        }
        Instruction::ArrayElement {
            operation,
            argument_count,
        } => {
            execute_array_element_instruction(stack, operation, *argument_count, span)?;
        }
        Instruction::ArrayMetadata {
            operation,
            argument_count,
        } => {
            execute_array_metadata_instruction(stack, operation, *argument_count, span)?;
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

#[allow(clippy::wildcard_imports)]
use super::*;

#[path = "application_call.rs"]
mod application_call;
pub use application_call::execute_call_instruction;

#[path = "application_multiple_values.rs"]
mod application_multiple_values;
pub use application_multiple_values::execute_multiple_value_call_instruction;

#[path = "application_list.rs"]
mod application_list;
pub use application_list::{
    execute_list_append_instruction, execute_list_construction_instruction,
    execute_list_construction_with_options_instruction,
};

#[path = "application_hash.rs"]
mod application_hash;
pub use application_hash::execute_hash_table_instruction;

#[path = "application_array.rs"]
mod application_array;
pub use application_array::{
    execute_array_element_instruction, execute_array_metadata_instruction,
};

#[path = "application_array_ops.rs"]
mod application_array_ops;
pub use application_array_ops::{
    execute_array_adjustment_instruction, execute_array_construction_instruction,
    execute_vector_construction_instruction,
};

#[path = "application_object.rs"]
mod application_object;
pub use application_object::{
    execute_evaluation_operation_instruction, execute_package_introspection_instruction,
    execute_package_listing_instruction, execute_package_mutation_instruction,
    execute_property_list_instruction, execute_symbol_binding_instruction,
    execute_symbol_creation_instruction, execute_symbol_function_instruction,
    execute_symbol_value_instruction,
};

#[path = "application_strings.rs"]
mod application_strings;
pub use application_strings::{
    execute_string_case_instruction, execute_string_comparison_instruction,
    execute_string_construction_instruction, execute_string_trim_instruction,
};

#[path = "application_object_system.rs"]
mod application_object_system;
pub use application_object_system::{
    execute_class_introspection_instruction, execute_condition_operation_instruction,
    execute_method_operation_instruction, execute_restart_operation_instruction,
    execute_slot_operation_instruction,
};

#[path = "application_io.rs"]
mod application_io;
pub use application_io::{
    execute_file_metadata_operation_instruction, execute_file_operation_instruction,
    execute_integer_operation_instruction, execute_stream_operation_instruction,
};

#[path = "application_sequence_ops.rs"]
mod application_sequence_ops;
pub use application_sequence_ops::{
    execute_sequence_concatenate_instruction, execute_sequence_conversion_instruction,
    execute_sequence_element_instruction, execute_sequence_length_instruction,
    execute_sequence_mutation_instruction, execute_sequence_subseq_instruction,
};

#[path = "application_sequences.rs"]
mod application_sequences;
pub use application_sequences::{
    execute_list_unary_instruction, execute_sequence_merge_instruction,
    execute_sequence_removal_instruction, execute_sequence_sort_instruction,
    execute_sequence_substitution_instruction, execute_sequence_unary_instruction,
};

#[path = "application_sequence_search.rs"]
mod application_sequence_search;
pub use application_sequence_search::{
    execute_association_search_instruction, execute_list_membership_instruction,
    execute_sequence_pair_search_instruction, execute_sequence_search_instruction,
};

#[path = "application_sequence_mapping.rs"]
mod application_sequence_mapping;
pub use application_sequence_mapping::{
    execute_list_mapping_instruction, execute_sequence_map_into_instruction,
    execute_sequence_mapping_instruction, execute_sequence_quantifier_instruction,
    execute_sequence_reduce_instruction,
};

#[path = "application_numeric.rs"]
mod application_numeric;
pub use application_numeric::{
    execute_numeric_binary_instruction, execute_numeric_bitfield_instruction,
    execute_numeric_boole_instruction, execute_numeric_comparison_instruction,
    execute_numeric_float_instruction, execute_numeric_fold_instruction,
    execute_numeric_random_instruction, execute_numeric_rounding_instruction,
    execute_numeric_unary_instruction,
};

#[path = "application_atoms.rs"]
mod application_atoms;
pub use application_atoms::{
    execute_character_comparison_instruction, execute_character_predicate_instruction,
    execute_character_unary_instruction, execute_equality_instruction,
    execute_symbol_unary_instruction, execute_type_predicate_instruction,
    execute_typep_instruction, execute_value_unary_instruction,
};

pub fn execute_apply_instruction(
    runtime: &Runtime,
    argument_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if argument_count == 0 || stack.len() < argument_count.saturating_add(1) {
        return Err(invalid("apply has too few stack values", span));
    }
    let arguments_start = stack.len() - argument_count;
    let mut evaluated = stack.split_off(arguments_start);
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("apply has no function value", span))?;
    let final_value = evaluated
        .pop()
        .ok_or_else(|| invalid("apply has no final list", span))?;
    let mut arguments = evaluated
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let mut final_arguments = final_value
        .primary_value()
        .list_items()
        .ok_or_else(|| invalid("apply's final argument must be a proper list", span))?;
    arguments.append(&mut final_arguments);
    stack.push(runtime.apply_in(
        &function_value.primary_value(),
        &arguments,
        span,
        environment,
    )?);
    Ok(())
}

pub fn execute_list_tail_instruction(
    operation: &str,
    option_count: usize,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count + 1 {
        return Err(invalid(
            "list tail operation has too few stack values",
            span,
        ));
    }
    let options = stack.split_off(stack.len() - option_count);
    let value = stack
        .pop()
        .ok_or_else(|| invalid("list tail operation has no list value", span))?;
    let mut arguments = vec![value];
    arguments.extend(options);
    let result = match operation {
        "LAST" => crate::builtins::last(&arguments),
        "BUTLAST" => crate::builtins::butlast(&arguments),
        "NBUTLAST" => crate::builtins::nbutlast(&arguments),
        _ => Err(invalid("unknown list tail operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_list_binary_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let second = stack
        .pop()
        .ok_or_else(|| invalid("binary list operation has too few stack values", span))?;
    let first = stack
        .pop()
        .ok_or_else(|| invalid("binary list operation has too few stack values", span))?;
    let result = match operation {
        "CONS" => crate::builtins::cons(&[first, second]),
        "NTH" => crate::builtins::nth(&[first, second]),
        "NTHCDR" => crate::builtins::nthcdr(&[first, second]),
        "RPLACA" => crate::builtins::rplaca(&[first, second]),
        "RPLACD" => crate::builtins::rplacd(&[first, second]),
        "TAILP" => crate::builtins::tailp(&[first, second]),
        "LDIFF" => crate::builtins::ldiff(&[first, second]),
        _ => Err(invalid("unknown binary list operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_vector_operation_instruction(
    operation: &str,
    argument_count: usize,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("vector operation has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let result = match operation {
        "FILL-POINTER" => crate::builtins::fill_pointer(&arguments),
        "VECTOR-PUSH" => crate::builtins::vector_push(&arguments),
        "VECTOR-PUSH-EXTEND" => crate::builtins::vector_push_extend(&arguments),
        "VECTOR-POP" => crate::builtins::vector_pop(&arguments),
        _ => Err(invalid("unknown vector operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_tree_equal_instruction(
    runtime: &Runtime,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("tree-equal has too few stack values", span));
    }
    let options = stack.split_off(stack.len() - option_count);
    let second = stack
        .pop()
        .ok_or_else(|| invalid("tree-equal has no second tree", span))?;
    let first = stack
        .pop()
        .ok_or_else(|| invalid("tree-equal has no first tree", span))?;
    stack.push(runtime.apply_tree_equal(&first, &second, &options, environment, span)?);
    Ok(())
}

pub fn execute_list_set_instruction(
    runtime: &Runtime,
    operation: &str,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("list set operation has too few stack values", span));
    }
    let options = stack.split_off(stack.len() - option_count);
    let second = stack
        .pop()
        .ok_or_else(|| invalid("list set operation has no second list", span))?;
    let first = stack
        .pop()
        .ok_or_else(|| invalid("list set operation has no first list", span))?;
    stack.push(runtime.apply_list_set_operation(
        operation,
        &first,
        &second,
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_character_element_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    span: Span,
) -> Result<(), RuntimeError> {
    let index = stack
        .pop()
        .ok_or_else(|| invalid("character-element has too few stack values", span))?;
    let string = stack
        .pop()
        .ok_or_else(|| invalid("character-element has too few stack values", span))?;
    let value = match operation {
        "CHAR" => crate::builtins::character(&[string, index])?,
        "SCHAR" => crate::builtins::simple_character(&[string, index])?,
        _ => return Err(invalid("unknown character-element operation", span)),
    };
    stack.push(value);
    Ok(())
}

pub fn execute_character_digit_predicate_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "character digit predicate has too few stack values",
            span,
        ));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::digit_character_p(&arguments)?);
    Ok(())
}

pub fn execute_character_digit_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("character digit has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::digit_character(&arguments)?);
    Ok(())
}

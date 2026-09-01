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

pub fn execute_property_list_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, environment: &Environment,
    operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("property-list operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = match operation {
        "GETF" => crate::builtins::getf(&arguments),
        "GET-PROPERTIES" => crate::builtins::get_properties(&arguments),
        "GET" | "PUTPROP" | "REMPROP" | "SYMBOL-PLIST" => runtime
            .apply_symbol_property_primitive(operation, &arguments, environment, span)
            .unwrap_or_else(|| Err(invalid("unknown property-list operation", span))),
        _ => Err(invalid("unknown property-list operation", span)),
    }?;
    stack.push(value); Ok(())
}

pub fn execute_symbol_value_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str, argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("symbol value operation has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count)
        .into_iter().map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_symbol_value_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown symbol value operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_symbol_binding_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, environment: &Environment,
    operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("symbol binding operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_symbol_property_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown symbol binding operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_symbol_function_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, environment: &Environment,
    operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("symbol function operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_symbol_function_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown symbol function operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_symbol_creation_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("symbol creation operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_symbol_creation_primitive(operation, &arguments, span)
        .unwrap_or_else(|| Err(invalid("unknown symbol creation operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_class_introspection_instruction(
    _runtime: &Runtime, stack: &mut Vec<Value>, environment: &Environment,
    operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("class introspection operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = Runtime::apply_class_introspection_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown class introspection operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_slot_operation_instruction(
    _runtime: &Runtime, stack: &mut Vec<Value>, operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("slot operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = Runtime::apply_slot_primitive(operation, &arguments, span)
        .unwrap_or_else(|| Err(invalid("unknown slot operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_condition_operation_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, environment: &Environment,
    operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("condition operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_condition_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown condition operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_restart_operation_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, environment: &Environment,
    operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("restart operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_restart_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown restart operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_method_operation_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, environment: &Environment,
    operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("method operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_method_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown method operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_evaluation_operation_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, environment: &Environment,
    operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("evaluation operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_evaluation_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown evaluation operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_package_introspection_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("package introspection operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_package_introspection_primitive(operation, &arguments, span)
        .unwrap_or_else(|| Err(invalid("unknown package introspection operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_package_mutation_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("package mutation operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = match operation {
        "USE-PACKAGE" | "UNUSE-PACKAGE" | "EXPORT" | "UNEXPORT" => runtime.apply_package_use_primitive(operation, &arguments, span),
        "IMPORT" | "SHADOWING-IMPORT" | "SHADOW" | "UNINTERN" => runtime.apply_package_symbol_primitive(operation, &arguments, span),
        _ => None,
    }.unwrap_or_else(|| Err(invalid("unknown package mutation operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_package_listing_instruction(
    runtime: &Runtime, stack: &mut Vec<Value>, operation: &str, argument_count: usize, span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count { return Err(invalid("package listing operation has too few stack values", span)); }
    let arguments = stack.split_off(stack.len() - argument_count).into_iter()
        .map(|value| value.primary_value()).collect::<Vec<_>>();
    let value = runtime.apply_package_listing_primitive(operation, &arguments, span)
        .unwrap_or_else(|| Err(invalid("unknown package listing operation", span)))?;
    stack.push(value);
    Ok(())
}

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

pub fn execute_list_mapping_instruction(
    runtime: &Runtime,
    operation: &str,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if sequence_count == 0 || stack.len() < sequence_count.saturating_add(1) {
        return Err(invalid(
            &format!(
                "{} has too few stack values",
                operation.to_ascii_lowercase()
            ),
            span,
        ));
    }
    let sequences_start = stack.len() - sequence_count;
    let sequences = stack.split_off(sequences_start);
    let function_value = stack.pop().ok_or_else(|| {
        invalid(
            &format!("{} has no function value", operation.to_ascii_lowercase()),
            span,
        )
    })?;
    let result = runtime.apply_list_mapping(
        operation,
        &function_value.primary_value(),
        &sequences,
        environment,
        span,
    )?;
    stack.push(result);
    Ok(())
}

pub fn execute_sequence_quantifier_instruction(
    runtime: &Runtime,
    operation: &str,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if sequence_count == 0 || stack.len() < sequence_count.saturating_add(1) {
        return Err(invalid(
            &format!("{} has too few stack values", operation.to_ascii_lowercase()),
            span,
        ));
    }
    let sequences_start = stack.len() - sequence_count;
    let sequences = stack.split_off(sequences_start);
    let predicate = stack.pop().ok_or_else(|| {
        invalid(
            &format!("{} has no predicate value", operation.to_ascii_lowercase()),
            span,
        )
    })?;
    stack.push(runtime.apply_sequence_quantifier(
        operation,
        &predicate.primary_value(),
        &sequences,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_mapping_instruction(
    runtime: &Runtime,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < sequence_count.saturating_add(2) {
        return Err(invalid("map has too few stack values", span));
    }
    let sequences_start = stack.len() - sequence_count;
    let sequences = stack.split_off(sequences_start);
    let function_value = stack.pop().ok_or_else(|| invalid("map has no function value", span))?;
    let result_type = stack.pop().ok_or_else(|| invalid("map has no result type", span))?;
    stack.push(runtime.apply_sequence_mapping(
        &result_type.primary_value(),
        &function_value.primary_value(),
        &sequences,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_map_into_instruction(
    runtime: &Runtime,
    sequence_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < sequence_count.saturating_add(2) {
        return Err(invalid("map-into has too few stack values", span));
    }
    let sequences_start = stack.len() - sequence_count;
    let sequences = stack.split_off(sequences_start);
    let function_value = stack.pop().ok_or_else(|| invalid("map-into has no function value", span))?;
    let destination = stack.pop().ok_or_else(|| invalid("map-into has no destination value", span))?;
    stack.push(runtime.apply_sequence_map_into(
        &destination.primary_value(),
        &function_value.primary_value(),
        &sequences,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_reduce_instruction(
    runtime: &Runtime,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("reduce has too few stack values", span));
    }
    let options_start = stack.len() - option_count;
    let options = stack.split_off(options_start);
    let sequence = stack
        .pop()
        .ok_or_else(|| invalid("reduce has no sequence value", span))?;
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("reduce has no function value", span))?;
    stack.push(runtime.apply_sequence_reduce(
        &function_value.primary_value(),
        &sequence.primary_value(),
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_merge_instruction(
    runtime: &Runtime,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(4) {
        return Err(invalid("merge has too few stack values", span));
    }
    let options_start = stack.len() - option_count;
    let options = stack.split_off(options_start);
    let predicate = stack.pop().ok_or_else(|| invalid("merge has no predicate value", span))?;
    let sequence2 = stack.pop().ok_or_else(|| invalid("merge has no second sequence value", span))?;
    let sequence1 = stack.pop().ok_or_else(|| invalid("merge has no first sequence value", span))?;
    let result_type = stack.pop().ok_or_else(|| invalid("merge has no result type value", span))?;
    stack.push(runtime.apply_sequence_merge_values(
        &result_type.primary_value(),
        &sequence1.primary_value(),
        &sequence2.primary_value(),
        &predicate.primary_value(),
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_sort_instruction(
    runtime: &Runtime,
    operation: &str,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("sort has too few stack values", span));
    }
    let options_start = stack.len() - option_count;
    let options = stack.split_off(options_start);
    let predicate = stack.pop().ok_or_else(|| invalid("sort has no predicate value", span))?;
    let sequence = stack.pop().ok_or_else(|| invalid("sort has no sequence value", span))?;
    stack.push(runtime.apply_sequence_sort(
        operation,
        &sequence.primary_value(),
        &predicate.primary_value(),
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_search_instruction(
    runtime: &Runtime,
    predicate: bool,
    operation: &str,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("sequence search has too few stack values", span));
    }
    let options_start = stack.len() - option_count;
    let options = stack.split_off(options_start);
    let sequence = stack.pop().ok_or_else(|| invalid("sequence search has no sequence", span))?;
    let first = stack.pop().ok_or_else(|| invalid("sequence search has no item or predicate", span))?;
    let result = if predicate {
        runtime.apply_sequence_search_if(operation, &first.primary_value(), &sequence.primary_value(), &options, environment, span)?
    } else {
        runtime.apply_sequence_search(operation, &first.primary_value(), &sequence.primary_value(), &options, environment, span)?
    };
    stack.push(result);
    Ok(())
}

pub fn execute_sequence_pair_search_instruction(
    runtime: &Runtime,
    operation: &str,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("sequence pair search has too few stack values", span));
    }
    let options = stack.split_off(stack.len() - option_count);
    let sequence2 = stack.pop().ok_or_else(|| invalid("sequence pair search has no second sequence", span))?;
    let sequence1 = stack.pop().ok_or_else(|| invalid("sequence pair search has no first sequence", span))?;
    stack.push(runtime.apply_sequence_pair_search(
        operation,
        &sequence1.primary_value(),
        &sequence2.primary_value(),
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_list_membership_instruction(
    runtime: &Runtime,
    operation: &str,
    _predicate: bool,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("list membership has too few stack values", span));
    }
    let options = stack.split_off(stack.len() - option_count);
    let list = stack.pop().ok_or_else(|| invalid("list membership has no list", span))?;
    let item_or_predicate = stack.pop().ok_or_else(|| invalid("list membership has no item or predicate", span))?;
    stack.push(runtime.apply_list_membership(
        operation,
        &item_or_predicate.primary_value(),
        &list.primary_value(),
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_association_search_instruction(
    runtime: &Runtime,
    operation: &str,
    _predicate: bool,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(2) {
        return Err(invalid("association search has too few stack values", span));
    }
    let options = stack.split_off(stack.len() - option_count);
    let alist = stack.pop().ok_or_else(|| invalid("association search has no alist", span))?;
    let item_or_predicate = stack.pop().ok_or_else(|| invalid("association search has no item or predicate", span))?;
    stack.push(runtime.apply_association_search(
        operation,
        &item_or_predicate.primary_value(),
        &alist.primary_value(),
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_removal_instruction(
    runtime: &Runtime,
    operation: &str,
    _predicate: bool,
    _duplicates: bool,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let required_values = if _duplicates { 1 } else { 2 };
    if stack.len() < option_count.saturating_add(required_values) {
        return Err(invalid("sequence removal has too few stack values", span));
    }
    let options = stack.split_off(stack.len() - option_count);
    let sequence = stack.pop().ok_or_else(|| invalid("sequence removal has no sequence", span))?;
    let item_or_predicate = if _duplicates {
        Value::Nil
    } else {
        stack.pop().ok_or_else(|| invalid("sequence removal has no item or predicate", span))?
    };
    stack.push(runtime.apply_sequence_remove(
        operation,
        &item_or_predicate.primary_value(),
        &sequence.primary_value(),
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_substitution_instruction(
    runtime: &Runtime,
    operation: &str,
    _predicate: bool,
    option_count: usize,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count.saturating_add(3) {
        return Err(invalid("sequence substitution has too few stack values", span));
    }
    let options = stack.split_off(stack.len() - option_count);
    let sequence = stack.pop().ok_or_else(|| invalid("sequence substitution has no sequence", span))?;
    let old_or_predicate = stack.pop().ok_or_else(|| invalid("sequence substitution has no old item or predicate", span))?;
    let new_item = stack.pop().ok_or_else(|| invalid("sequence substitution has no new item", span))?;
    stack.push(runtime.apply_sequence_substitute_values(
        operation,
        &new_item,
        &old_or_predicate,
        &sequence,
        &options,
        environment,
        span,
    )?);
    Ok(())
}

pub fn execute_sequence_unary_instruction(
    runtime: &Runtime,
    operation: &str,
    stack: &mut Vec<Value>,
    environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("unary sequence operation has too few stack values", span))?;
    let result = match operation {
        "COPY-TREE" => Ok(runtime.copy_tree(&value)),
        "COPY-SEQ" => crate::builtins::copy_seq(&[value]),
        "REVERSE" | "NREVERSE" => runtime.apply_sequence_reverse(&value, span),
        _ => Err(invalid("unknown unary sequence operation", span)),
    }?;
    let _ = environment;
    stack.push(result);
    Ok(())
}

pub fn execute_list_unary_instruction(
    _runtime: &Runtime,
    operation: &str,
    stack: &mut Vec<Value>,
    _environment: &Environment,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("unary list operation has too few stack values", span))?;
    let result = match operation {
        "CAR" => crate::builtins::car(&[value]),
        "CDR" => crate::builtins::cdr(&[value]),
        "FIRST" => crate::builtins::first(&[value]),
        "REST" => crate::builtins::rest(&[value]),
        "COPY-LIST" => crate::builtins::copy_list(&[value]),
        "COPY-ALIST" => crate::builtins::copy_alist(&[value]),
        "ENDP" => crate::builtins::endp(&[value]),
        "LIST-LENGTH" => crate::builtins::list_length(&[value]),
        "VALUES-LIST" => crate::builtins::values_list(&[value]),
        "SECOND" => crate::builtins::nth(&[Value::Integer(1), value]),
        "THIRD" => crate::builtins::nth(&[Value::Integer(2), value]),
        "FOURTH" => crate::builtins::nth(&[Value::Integer(3), value]),
        "FIFTH" => crate::builtins::nth(&[Value::Integer(4), value]),
        "SIXTH" => crate::builtins::nth(&[Value::Integer(5), value]),
        "SEVENTH" => crate::builtins::nth(&[Value::Integer(6), value]),
        "EIGHTH" => crate::builtins::nth(&[Value::Integer(7), value]),
        "NINTH" => crate::builtins::nth(&[Value::Integer(8), value]),
        "TENTH" => crate::builtins::nth(&[Value::Integer(9), value]),
        _ => Err(invalid("unknown unary list operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_character_unary_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("unary character operation has too few stack values", span))?;
    let result = match operation {
        "CHARACTER" => crate::builtins::character_value(&[value]),
        "CHAR-CODE" => crate::builtins::char_code(&[value]),
        "CHAR-INT" => crate::builtins::char_int(&[value]),
        "CODE-CHAR" => crate::builtins::code_char(&[value]),
        "INT-CHAR" => crate::builtins::int_char(&[value]),
        "CHAR-UPCASE" => crate::builtins::character_upcase(&[value]),
        "CHAR-DOWNCASE" => crate::builtins::character_downcase(&[value]),
        "CHAR-NAME" => crate::builtins::character_name(&[value]),
        "NAME-CHAR" => crate::builtins::name_character(&[value]),
        _ => Err(invalid("unknown unary character operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_unary_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("unary numeric operation has too few stack values", span))?;
    let result = match operation {
        "1+" => crate::builtins::increment(&[value]),
        "1-" => crate::builtins::decrement(&[value]),
        "ABS" => crate::builtins::absolute(&[value]),
        "SIGNUM" => crate::builtins::signum(&[value]),
        "ZEROP" => crate::builtins::zerop(&[value]),
        "PLUSP" => crate::builtins::plusp(&[value]),
        "MINUSP" => crate::builtins::minusp(&[value]),
        "EVENP" => crate::builtins::evenp(&[value]),
        "ODDP" => crate::builtins::oddp(&[value]),
        "LOGNOT" => crate::builtins::lognot(&[value]),
        "LOGCOUNT" => crate::builtins::logcount(&[value]),
        "INTEGER-LENGTH" => crate::builtins::integer_length(&[value]),
        "ISQRT" => crate::builtins::integer_square_root_builtin(&[value]),
        "SQRT" => crate::builtins::square_root(&[value]),
        "SIN" => crate::builtins::sine(&[value]),
        "COS" => crate::builtins::cosine(&[value]),
        "CIS" => crate::builtins::cis(&[value]),
        "TAN" => crate::builtins::tangent(&[value]),
        "EXP" => crate::builtins::exponential(&[value]),
        "ASIN" => crate::builtins::arc_sine(&[value]),
        "ACOS" => crate::builtins::arc_cosine(&[value]),
        "SINH" => crate::builtins::hyperbolic_sine(&[value]),
        "COSH" => crate::builtins::hyperbolic_cosine(&[value]),
        "TANH" => crate::builtins::hyperbolic_tangent(&[value]),
        "REALPART" => crate::builtins::real_part(&[value]),
        "IMAGPART" => crate::builtins::imaginary_part(&[value]),
        "CONJUGATE" => crate::builtins::conjugate(&[value]),
        "PHASE" => crate::builtins::phase(&[value]),
        "RATIONAL" => crate::builtins::rational(&[value]),
        "RATIONALIZE" => crate::builtins::rationalize(&[value]),
        _ => Err(invalid("unknown unary numeric operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_rounding_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("numeric rounding has too few stack values", span));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.drain(start..).collect::<Vec<_>>();
    let result = match operation {
        "FLOOR" => crate::builtins::floor(&arguments),
        "CEILING" => crate::builtins::ceiling(&arguments),
        "TRUNCATE" => crate::builtins::truncate(&arguments),
        "ROUND" => crate::builtins::round(&arguments),
        _ => Err(invalid("unknown numeric rounding operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_comparison_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("numeric comparison has too few stack values", span));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.drain(start..).collect::<Vec<_>>();
    let result = match operation {
        "=" => crate::builtins::numeric_equal(&arguments),
        "/=" => crate::builtins::numeric_not_equal(&arguments),
        "<" => crate::builtins::less_than(&arguments),
        ">" => crate::builtins::greater_than(&arguments),
        "<=" => crate::builtins::less_equal(&arguments),
        ">=" => crate::builtins::greater_equal(&arguments),
        _ => Err(invalid("unknown numeric comparison operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_fold_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("numeric fold has too few stack values", span));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.drain(start..).collect::<Vec<_>>();
    let result = match operation {
        "MIN" => crate::builtins::minimum(&arguments),
        "MAX" => crate::builtins::maximum(&arguments),
        "GCD" => crate::builtins::greatest_common_divisor(&arguments),
        "LCM" => crate::builtins::least_common_multiple(&arguments),
        "LOGAND" => crate::builtins::logand(&arguments),
        "LOGIOR" => crate::builtins::logior(&arguments),
        "LOGXOR" => crate::builtins::logxor(&arguments),
        _ => Err(invalid("unknown numeric fold operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_binary_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    span: Span,
) -> Result<(), RuntimeError> {
    let right = stack.pop().ok_or_else(|| invalid("numeric binary operation has too few stack values", span))?;
    let left = stack.pop().ok_or_else(|| invalid("numeric binary operation has too few stack values", span))?;
    let result = match operation {
        "MOD" => crate::builtins::modulo(&[left, right]),
        "REM" => crate::builtins::remainder(&[left, right]),
        "ASH" => crate::builtins::arithmetic_shift(&[left, right]),
        "LOGTEST" => crate::builtins::logtest(&[left, right]),
        "LOGANDC1" => crate::builtins::logandc1(&[left, right]),
        "LOGANDC2" => crate::builtins::logandc2(&[left, right]),
        "LOGEQV" => crate::builtins::logeqv(&[left, right]),
        "LOGNAND" => crate::builtins::lognand(&[left, right]),
        "LOGNOR" => crate::builtins::lognor(&[left, right]),
        "LOGORC1" => crate::builtins::logorc1(&[left, right]),
        "LOGORC2" => crate::builtins::logorc2(&[left, right]),
        "LOGBITP" => crate::builtins::logbitp(&[left, right]),
        "EXPT" => crate::builtins::exponentiate(&[left, right]),
        _ => Err(invalid("unknown numeric binary operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_boole_instruction(
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let right = stack.pop().ok_or_else(|| invalid("BOOLE has too few stack values", span))?;
    let left = stack.pop().ok_or_else(|| invalid("BOOLE has too few stack values", span))?;
    let operation = stack.pop().ok_or_else(|| invalid("BOOLE has too few stack values", span))?;
    let result = crate::builtins::boole(&[operation, left, right])?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_bitfield_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("numeric bitfield operation has too few stack values", span));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.split_off(start);
    let result = match operation {
        "BYTE" => crate::builtins::byte(&arguments),
        "LDB" => crate::builtins::ldb(&arguments),
        "MASK-FIELD" => crate::builtins::mask_field(&arguments),
        "DPB" => crate::builtins::dpb(&arguments),
        "DEPOSIT-FIELD" => crate::builtins::deposit_field(&arguments),
        _ => Err(invalid("unknown numeric bitfield operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_numeric_float_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("numeric float operation has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let result = match operation {
        "FLOAT" => crate::builtins::float_value(&arguments),
        "FLOAT-SIGN" => crate::builtins::float_sign(&arguments),
        "FLOAT-DIGITS" => crate::builtins::float_digits(&arguments),
        "FLOAT-PRECISION" => crate::builtins::float_precision(&arguments),
        "FLOAT-RADIX" => crate::builtins::float_radix(&arguments),
        "SCALE-FLOAT" => crate::builtins::scale_float(&arguments),
        "DECODE-FLOAT" => crate::builtins::decode_float(&arguments),
        "INTEGER-DECODE-FLOAT" => crate::builtins::integer_decode_float(&arguments),
        _ => Err(invalid("unknown numeric float operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_character_comparison_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("character comparison has too few stack values", span));
    }
    let start = stack.len() - argument_count;
    let arguments = stack.drain(start..).map(|value| value.primary_value()).collect::<Vec<_>>();
    let result = match operation {
        "CHAR=" => crate::builtins::character_equal(&arguments),
        "CHAR/=" => crate::builtins::character_not_equal(&arguments),
        "CHAR-EQUAL" => crate::builtins::character_case_equal(&arguments),
        "CHAR-NOT-EQUAL" => crate::builtins::character_case_not_equal(&arguments),
        "CHAR<" => crate::builtins::character_less_than(&arguments),
        "CHAR>" => crate::builtins::character_greater_than(&arguments),
        "CHAR<=" => crate::builtins::character_less_equal(&arguments),
        "CHAR>=" => crate::builtins::character_greater_equal(&arguments),
        "CHAR-LESSP" => crate::builtins::character_case_less_than(&arguments),
        "CHAR-GREATERP" => crate::builtins::character_case_greater_than(&arguments),
        "CHAR-NOT-LESSP" => crate::builtins::character_case_greater_equal(&arguments),
        "CHAR-NOT-GREATERP" => crate::builtins::character_case_less_equal(&arguments),
        _ => Err(invalid("unknown character comparison operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_type_predicate_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack.pop().ok_or_else(|| invalid("type predicate has too few stack values", span))?;
    let result = match operation {
        "ATOM" => crate::builtins::atom(&[value]),
        "CONSP" => crate::builtins::consp(&[value]),
        "LISTP" => crate::builtins::listp(&[value]),
        "NUMBERP" => crate::builtins::numberp(&[value]),
        "COMPLEXP" => crate::builtins::complexp(&[value]),
        "INTEGERP" => crate::builtins::integerp(&[value]),
        "FLOATP" => crate::builtins::floatp(&[value]),
        "RATIONALP" => crate::builtins::rationalp(&[value]),
        "STRINGP" => crate::builtins::stringp(&[value]),
        "SIMPLE-STRING-P" => crate::builtins::simple_string_p(&[value]),
        "CHARACTERP" => crate::builtins::characterp(&[value]),
        "SYMBOLP" => crate::builtins::symbolp(&[value]),
        "PACKAGEP" => crate::builtins::packagep(&[value]),
        "KEYWORDP" => crate::builtins::keywordp(&[value]),
        "VECTORP" => crate::builtins::vectorp(&[value]),
        "FUNCTIONP" => crate::builtins::functionp(&[value]),
        "SIMPLE-VECTOR-P" => crate::builtins::simple_vector_p(&[value]),
        "BIT-VECTOR-P" => crate::builtins::bit_vector_p(&[value]),
        "SIMPLE-BIT-VECTOR-P" => crate::builtins::simple_bit_vector_p(&[value]),
        "ARRAYP" => crate::builtins::arrayp(&[value]),
        "SIMPLE-ARRAY-P" => crate::builtins::simple_array_p(&[value]),
        "HASH-TABLE-P" => crate::builtins::hash_table_p(&[value]),
        "RANDOM-STATE-P" => crate::builtins::random_state_p(&[value]),
        "ALPHA-CHAR-P" => crate::builtins::alpha_character_p(&[value]),
        "ALPHANUMERICP" => crate::builtins::alphanumeric_p(&[value]),
        "GRAPHIC-CHAR-P" => crate::builtins::graphic_character_p(&[value]),
        "STANDARD-CHAR-P" => crate::builtins::standard_character_p(&[value]),
        "UPPER-CASE-P" => crate::builtins::upper_case_p(&[value]),
        "LOWER-CASE-P" => crate::builtins::lower_case_p(&[value]),
        "BOTH-CASE-P" => crate::builtins::both_case_p(&[value]),
        "DIGIT-CHAR-P" => crate::builtins::digit_character_p(&[value]),
        "STREAMP" => crate::builtins::streamp(&[value]),
        "INPUT-STREAM-P" => crate::builtins::input_stream_p(&[value]),
        "OUTPUT-STREAM-P" => crate::builtins::output_stream_p(&[value]),
        _ => Err(invalid("unknown type predicate operation", span)),
    }?;
    stack.push(result);
    Ok(())
}

pub fn execute_equality_instruction(
    operation: &str,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let right = stack.pop().ok_or_else(|| invalid("equality predicate has too few stack values", span))?;
    let left = stack.pop().ok_or_else(|| invalid("equality predicate has too few stack values", span))?;
    let result = match operation {
        "EQ" => left.eq_value(&right),
        "EQL" => crate::builtins::eql_value(&left, &right),
        "EQUAL" => left.equal_value(&right),
        "EQUALP" => crate::builtins::equalp_value(&left, &right),
        _ => return Err(invalid("unknown equality predicate operation", span)),
    };
    stack.push(Value::boolean(result));
    Ok(())
}

pub fn execute_list_tail_instruction(
    operation: &str,
    option_count: usize,
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < option_count + 1 {
        return Err(invalid("list tail operation has too few stack values", span));
    }
    let options = stack.split_off(stack.len() - option_count);
    let value = stack.pop().ok_or_else(|| invalid("list tail operation has no list value", span))?;
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
    let list = stack.pop().ok_or_else(|| invalid("binary list operation has too few stack values", span))?;
    let index = stack.pop().ok_or_else(|| invalid("binary list operation has too few stack values", span))?;
    let result = match operation {
        "NTH" => crate::builtins::nth(&[index, list]),
        "NTHCDR" => crate::builtins::nthcdr(&[index, list]),
        _ => Err(invalid("unknown binary list operation", span)),
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

pub fn execute_sequence_length_instruction(
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let value = stack
        .pop()
        .ok_or_else(|| invalid("sequence-length has too few stack values", span))?;
    stack.push(crate::builtins::length(&[value])?);
    Ok(())
}

pub fn execute_sequence_element_instruction(
    stack: &mut Vec<Value>,
    span: Span,
) -> Result<(), RuntimeError> {
    let index = stack
        .pop()
        .ok_or_else(|| invalid("sequence-element has too few stack values", span))?;
    let sequence = stack
        .pop()
        .ok_or_else(|| invalid("sequence-element has too few stack values", span))?;
    stack.push(crate::builtins::elt(&[sequence, index])?);
    Ok(())
}

pub fn execute_sequence_subseq_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("sequence-subseq has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::subseq(&arguments)?);
    Ok(())
}

pub fn execute_sequence_mutation_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("sequence mutation has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let value = match operation {
        "FILL" => crate::builtins::fill(&arguments)?,
        "REPLACE" => crate::builtins::replace(&arguments)?,
        _ => return Err(invalid("unknown sequence mutation operation", span)),
    };
    stack.push(value);
    Ok(())
}

pub fn execute_sequence_concatenate_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("sequence-concatenate has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::concatenate(&arguments)?);
    Ok(())
}

pub fn execute_sequence_conversion_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("sequence conversion has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let value = match operation {
        "MAKE-SEQUENCE" => crate::builtins::make_sequence(&arguments)?,
        "COERCE" => crate::builtins::coerce(&arguments)?,
        _ => return Err(invalid("unknown sequence conversion operation", span)),
    };
    stack.push(value);
    Ok(())
}

pub fn execute_vector_construction_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("vector construction has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::vector(&arguments)?);
    Ok(())
}

pub fn execute_array_construction_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("array construction has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::make_array(&arguments)?);
    Ok(())
}

pub fn execute_array_adjustment_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("array adjustment has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::adjust_array(&arguments)?);
    Ok(())
}

pub fn execute_string_case_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("string case has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let value = match operation {
        "STRING-UPCASE" => crate::builtins::string_upcase(&arguments)?,
        "STRING-DOWNCASE" => crate::builtins::string_downcase(&arguments)?,
        "STRING-CAPITALIZE" => crate::builtins::string_capitalize(&arguments)?,
        "NSTRING-UPCASE" => crate::builtins::nstring_upcase(&arguments)?,
        "NSTRING-DOWNCASE" => crate::builtins::nstring_downcase(&arguments)?,
        "NSTRING-CAPITALIZE" => crate::builtins::nstring_capitalize(&arguments)?,
        _ => return Err(invalid("unknown string case operation", span)),
    };
    stack.push(value);
    Ok(())
}

pub fn execute_string_comparison_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < 2 {
        return Err(invalid("string comparison has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - 2)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = match operation {
        "STRING=" => crate::builtins::string_equal(&arguments)?,
        "STRING-EQUAL" => crate::builtins::string_case_equal(&arguments)?,
        "STRING<" => crate::builtins::string_less_than(&arguments)?,
        "STRING>" => crate::builtins::string_greater_than(&arguments)?,
        "STRING<=" => crate::builtins::string_less_equal(&arguments)?,
        "STRING>=" => crate::builtins::string_greater_equal(&arguments)?,
        _ => return Err(invalid("unknown string comparison operation", span)),
    };
    stack.push(value);
    Ok(())
}

pub fn execute_string_trim_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < 2 {
        return Err(invalid("string trim has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - 2)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = match operation {
        "STRING-TRIM" => crate::builtins::string_trim(&arguments)?,
        "STRING-LEFT-TRIM" => crate::builtins::string_left_trim(&arguments)?,
        "STRING-RIGHT-TRIM" => crate::builtins::string_right_trim(&arguments)?,
        _ => return Err(invalid("unknown string trim operation", span)),
    };
    stack.push(value);
    Ok(())
}

pub fn execute_string_construction_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("string construction has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    let value = match operation {
        "STRING" => crate::builtins::string_value(&arguments),
        "MAKE-STRING" => crate::builtins::make_string(&arguments),
        _ => return Err(invalid("unknown string construction operation", span)),
    }?;
    stack.push(value);
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
        return Err(invalid("character digit predicate has too few stack values", span));
    }
    let arguments = stack.split_off(stack.len() - argument_count);
    stack.push(crate::builtins::digit_character_p(&arguments)?);
    Ok(())
}

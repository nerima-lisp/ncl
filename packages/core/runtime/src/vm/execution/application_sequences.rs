#[allow(clippy::wildcard_imports)]
use super::*;

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
            &format!(
                "{} has too few stack values",
                operation.to_ascii_lowercase()
            ),
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
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("map has no function value", span))?;
    let result_type = stack
        .pop()
        .ok_or_else(|| invalid("map has no result type", span))?;
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
    let function_value = stack
        .pop()
        .ok_or_else(|| invalid("map-into has no function value", span))?;
    let destination = stack
        .pop()
        .ok_or_else(|| invalid("map-into has no destination value", span))?;
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
    let predicate = stack
        .pop()
        .ok_or_else(|| invalid("merge has no predicate value", span))?;
    let sequence2 = stack
        .pop()
        .ok_or_else(|| invalid("merge has no second sequence value", span))?;
    let sequence1 = stack
        .pop()
        .ok_or_else(|| invalid("merge has no first sequence value", span))?;
    let result_type = stack
        .pop()
        .ok_or_else(|| invalid("merge has no result type value", span))?;
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
    let predicate = stack
        .pop()
        .ok_or_else(|| invalid("sort has no predicate value", span))?;
    let sequence = stack
        .pop()
        .ok_or_else(|| invalid("sort has no sequence value", span))?;
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
    let sequence = stack
        .pop()
        .ok_or_else(|| invalid("sequence search has no sequence", span))?;
    let first = stack
        .pop()
        .ok_or_else(|| invalid("sequence search has no item or predicate", span))?;
    let result = if predicate {
        runtime.apply_sequence_search_if(
            operation,
            &first.primary_value(),
            &sequence.primary_value(),
            &options,
            environment,
            span,
        )?
    } else {
        runtime.apply_sequence_search(
            operation,
            &first.primary_value(),
            &sequence.primary_value(),
            &options,
            environment,
            span,
        )?
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
        return Err(invalid(
            "sequence pair search has too few stack values",
            span,
        ));
    }
    let options = stack.split_off(stack.len() - option_count);
    let sequence2 = stack
        .pop()
        .ok_or_else(|| invalid("sequence pair search has no second sequence", span))?;
    let sequence1 = stack
        .pop()
        .ok_or_else(|| invalid("sequence pair search has no first sequence", span))?;
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
    let list = stack
        .pop()
        .ok_or_else(|| invalid("list membership has no list", span))?;
    let item_or_predicate = stack
        .pop()
        .ok_or_else(|| invalid("list membership has no item or predicate", span))?;
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
    let alist = stack
        .pop()
        .ok_or_else(|| invalid("association search has no alist", span))?;
    let item_or_predicate = stack
        .pop()
        .ok_or_else(|| invalid("association search has no item or predicate", span))?;
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
    let sequence = stack
        .pop()
        .ok_or_else(|| invalid("sequence removal has no sequence", span))?;
    let item_or_predicate = if _duplicates {
        Value::Nil
    } else {
        stack
            .pop()
            .ok_or_else(|| invalid("sequence removal has no item or predicate", span))?
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
        return Err(invalid(
            "sequence substitution has too few stack values",
            span,
        ));
    }
    let options = stack.split_off(stack.len() - option_count);
    let sequence = stack
        .pop()
        .ok_or_else(|| invalid("sequence substitution has no sequence", span))?;
    let old_or_predicate = stack
        .pop()
        .ok_or_else(|| invalid("sequence substitution has no old item or predicate", span))?;
    let new_item = stack
        .pop()
        .ok_or_else(|| invalid("sequence substitution has no new item", span))?;
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


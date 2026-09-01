#[allow(clippy::wildcard_imports)]
use super::*;

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


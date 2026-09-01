#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_list_construction_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    dotted: bool,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("list construction has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    stack.push(if dotted {
        crate::builtins::list_star(&arguments)?
    } else {
        crate::builtins::list(&arguments)?
    });
    Ok(())
}

pub fn execute_list_construction_with_options_instruction(
    stack: &mut Vec<Value>,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("MAKE-LIST has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    stack.push(crate::builtins::make_list(&arguments)?);
    Ok(())
}

pub fn execute_list_append_instruction(
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("list append has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = match operation {
        "APPEND" => crate::builtins::append(&arguments),
        "NCONC" => crate::builtins::nconc(&arguments),
        "REVAPPEND" => crate::builtins::revappend(&arguments),
        "NRECONC" => crate::builtins::nreconc(&arguments),
        "ACONS" => crate::builtins::acons(&arguments),
        "PAIRLIS" => crate::builtins::pairlis(&arguments),
        _ => Err(invalid("unknown list append operation", span)),
    }?;
    stack.push(value);
    Ok(())
}

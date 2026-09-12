#[allow(clippy::wildcard_imports)]
use super::*;

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
        "STRING-TRIM" => crate::builtins::string_trim(&arguments),
        "STRING-LEFT-TRIM" => crate::builtins::string_left_trim(&arguments),
        "STRING-RIGHT-TRIM" => crate::builtins::string_right_trim(&arguments),
        _ => return Err(invalid("unknown string trim operation", span)),
    }?;
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
        return Err(invalid(
            "string construction has too few stack values",
            span,
        ));
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

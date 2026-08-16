
fn jump_target(function: &FunctionCode, target: usize, span: Span) -> Result<usize, RuntimeError> {
    if target >= function.instructions.len() {
        return Err(invalid("compiled jump target is out of range", span));
    }
    Ok(target)
}


fn invalid(message: &str, span: Span) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: message.to_string(),
        span: Some(span),
    }
}

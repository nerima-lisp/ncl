use super::super::*;

pub(super) fn validate_setf_items(items: &[Form], span: Span) -> Result<&[Form], CompileError> {
    if items.len() < 3 || items.len().is_multiple_of(2) {
        return Err(CompileError::new(
            CompileErrorKind::InvalidForm {
                message: "setf needs place/value pairs".to_string(),
            },
            operator_span(items, span),
        ));
    }
    Ok(items.get(1..).unwrap_or(&[]))
}

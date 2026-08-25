use super::*;

impl CompileState {
    pub(super) fn require_arity(
        &self,
        items: &[Form],
        operator: &str,
        expected: &str,
        expected_count: usize,
        span: Span,
    ) -> Result<(), CompileError> {
        if items.len().saturating_sub(1) != expected_count {
            return Err(self.arity_error(items, operator, expected, span));
        }
        Ok(())
    }

    pub(super) fn arity_error(
        &self,
        items: &[Form],
        operator: &str,
        expected: &str,
        span: Span,
    ) -> CompileError {
        CompileError::new(
            CompileErrorKind::Arity {
                operator: operator.to_string(),
                expected: expected.to_string(),
                actual: items.len().saturating_sub(1),
            },
            span,
        )
    }

    pub(super) fn internal_error(&self, span: Span, message: &str) -> CompileError {
        CompileError::new(
            CompileErrorKind::Internal {
                message: message.to_string(),
            },
            span,
        )
    }
}

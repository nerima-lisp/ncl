use crate::{CompileError, CompileErrorKind, CompileState, Form, Span};

impl CompileState {
    pub(crate) fn require_arity(
        items: &[Form],
        operator: &str,
        expected: &str,
        expected_count: usize,
        span: Span,
    ) -> Result<(), CompileError> {
        if items.len().saturating_sub(1) != expected_count {
            return Err(Self::arity_error(items, operator, expected, span));
        }
        Ok(())
    }

    pub(crate) fn arity_error(
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

    pub(crate) fn internal_error(span: Span, message: &str) -> CompileError {
        CompileError::new(
            CompileErrorKind::Internal {
                message: message.to_string(),
            },
            span,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_error_preserves_message_and_span() {
        let error = CompileState::internal_error(Span::new(4, 9), "invariant failed");

        assert_eq!(error.span, Span::new(4, 9));
        assert!(matches!(
            error.kind,
            CompileErrorKind::Internal { message } if message == "invariant failed"
        ));
    }
}

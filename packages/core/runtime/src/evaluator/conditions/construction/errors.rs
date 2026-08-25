use ncl_syntax::Span;

use crate::environment::normalize_name;
use crate::{RuntimeError, Value};

pub(super) fn display_initarg(name: &str, escaped: bool) -> String {
    if escaped {
        format!(":|{name}|")
    } else {
        format!(":{}", normalize_name(name))
    }
}

pub(super) fn type_error(expected: &str, value: &Value, span: Span) -> RuntimeError {
    RuntimeError::Type {
        expected: expected.to_owned(),
        actual: value.type_name().to_owned(),
        span: Some(span),
    }
}

pub(super) fn invalid(message: impl Into<String>, span: Span) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: message.into(),
        span: Some(span),
    }
}

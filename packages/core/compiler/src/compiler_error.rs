use std::error::Error;
use std::fmt;

use ncl_syntax::Span;

/// The category of a compile-time error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileErrorKind {
    /// An operator received the wrong number of arguments.
    Arity {
        /// Operator name.
        operator: String,
        /// Human-readable expected arity.
        expected: String,
        /// Actual argument count.
        actual: usize,
    },
    /// A list was required.
    ExpectedList {
        /// Context requiring the list.
        context: String,
    },
    /// A symbol was required.
    ExpectedSymbol {
        /// Context requiring the symbol.
        context: String,
    },
    /// The form is malformed.
    InvalidForm {
        /// Diagnostic message.
        message: String,
    },
    /// The form is valid syntax but unsupported by the compiler.
    UnsupportedForm {
        /// Diagnostic message.
        message: String,
    },
    /// An internal compiler invariant failed.
    Internal {
        /// Diagnostic message.
        message: String,
    },
}

/// A typed compiler error tied to the source span that caused it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileError {
    /// Structured error category.
    pub kind: CompileErrorKind,
    /// Source span associated with the error.
    pub span: Span,
}

impl CompileError {
    /// Construct a compiler error at a source span.
    #[must_use]
    pub const fn new(kind: CompileErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for CompileErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity {
                operator,
                expected,
                actual,
            } => write!(
                formatter,
                "{operator} expected {expected} arguments, received {actual}"
            ),
            Self::ExpectedList { context } => write!(formatter, "{context} must be a list"),
            Self::ExpectedSymbol { context } => write!(formatter, "{context} must be a symbol"),
            Self::InvalidForm { message }
            | Self::UnsupportedForm { message }
            | Self::Internal { message } => formatter.write_str(message),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at byte {}..{}",
            self.kind, self.span.start, self.span.end
        )
    }
}

impl Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::{CompileError, CompileErrorKind};
    use ncl_syntax::Span;

    #[test]
    fn formats_each_error_category() {
        let cases = [
            (
                CompileErrorKind::Arity {
                    operator: "+".into(),
                    expected: "2".into(),
                    actual: 1,
                },
                "+ expected 2 arguments, received 1",
            ),
            (
                CompileErrorKind::ExpectedList {
                    context: "body".into(),
                },
                "body must be a list",
            ),
            (
                CompileErrorKind::ExpectedSymbol {
                    context: "name".into(),
                },
                "name must be a symbol",
            ),
            (
                CompileErrorKind::InvalidForm {
                    message: "invalid".into(),
                },
                "invalid",
            ),
            (
                CompileErrorKind::UnsupportedForm {
                    message: "unsupported".into(),
                },
                "unsupported",
            ),
            (
                CompileErrorKind::Internal {
                    message: "internal".into(),
                },
                "internal",
            ),
        ];

        for (kind, expected) in cases {
            assert_eq!(kind.to_string(), expected);
            assert_eq!(
                CompileError::new(kind, Span::new(2, 4)).to_string(),
                format!("{expected} at byte 2..4")
            );
        }
    }
}

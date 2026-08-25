use std::error::Error;
use std::fmt;

use ncl_syntax::Span;

/// The category of a compile-time error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileErrorKind {
    Arity {
        operator: String,
        expected: String,
        actual: usize,
    },
    ExpectedList {
        context: String,
    },
    ExpectedSymbol {
        context: String,
    },
    InvalidForm {
        message: String,
    },
    UnsupportedForm {
        message: String,
    },
    Internal {
        message: String,
    },
}

/// A typed compiler error tied to the source span that caused it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub span: Span,
}

impl CompileError {
    #[must_use]
    pub fn new(kind: CompileErrorKind, span: Span) -> Self {
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
            Self::ExpectedSymbol { context } => {
                write!(formatter, "{context} must be a symbol")
            }
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
    use super::*;

    #[test]
    fn displays_every_compile_error_kind() {
        let cases = [
            (
                CompileErrorKind::Arity {
                    operator: "if".into(),
                    expected: "2 or 3".into(),
                    actual: 1,
                },
                "if expected 2 or 3 arguments, received 1",
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
        }
    }

    #[test]
    fn constructs_and_displays_span_aware_error() {
        let error = CompileError::new(
            CompileErrorKind::Internal {
                message: "broken".into(),
            },
            Span::new(4, 9),
        );

        assert_eq!(error.span, Span::new(4, 9));
        assert_eq!(error.to_string(), "broken at byte 4..9");
        let source_error: &dyn Error = &error;
        assert_eq!(source_error.to_string(), error.to_string());
    }
}

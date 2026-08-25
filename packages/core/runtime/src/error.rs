use std::error::Error;
use std::fmt;

use ncl_compiler::CompileError;
use ncl_syntax::{ReadError, Span};

use crate::Value;

#[derive(Clone, Debug)]
pub struct ReturnValue(Value);

impl ReturnValue {
    #[must_use]
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn into_value(self) -> Value {
        self.0
    }
}

impl PartialEq for ReturnValue {
    fn eq(&self, other: &Self) -> bool {
        self.0.equal_value(&other.0)
    }
}

impl Eq for ReturnValue {}

#[derive(Clone, Debug)]
pub struct ThrowTag(Value);

impl ThrowTag {
    pub(crate) fn new(value: Value) -> Self {
        Self(value)
    }

    pub(crate) fn matches(&self, value: &Value) -> bool {
        self.0.eq_value(value)
    }
}

impl PartialEq for ThrowTag {
    fn eq(&self, other: &Self) -> bool {
        self.matches(&other.0)
    }
}

impl Eq for ThrowTag {}

impl fmt::Display for ThrowTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    Read(ReadError),
    Compile(CompileError),
    UnboundVariable {
        name: String,
        span: Option<Span>,
    },
    NotCallable {
        value: String,
        span: Option<Span>,
    },
    Arity {
        function: String,
        expected: String,
        actual: usize,
    },
    Type {
        expected: String,
        actual: String,
        span: Option<Span>,
    },
    InvalidForm {
        message: String,
        span: Option<Span>,
    },
    Signaled(Box<SignaledError>),
    Package {
        message: String,
        span: Option<Span>,
    },
    ReturnFrom {
        block: String,
        target: Option<u64>,
        value: ReturnValue,
        span: Option<Span>,
    },
    Go {
        tag: String,
        target: Option<u64>,
        span: Option<Span>,
    },
    Throw {
        tag: ThrowTag,
        value: ReturnValue,
        span: Option<Span>,
    },
    InvokeRestart {
        name: String,
        value: ReturnValue,
        arguments: Vec<ReturnValue>,
        span: Option<Span>,
    },
    DivisionByZero,
    NumericOverflow,
    Io(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaledError {
    pub(crate) condition: String,
    pub(crate) condition_types: Vec<String>,
    pub(crate) message: String,
    pub(crate) format_control: Option<String>,
    pub(crate) format_arguments: Vec<ReturnValue>,
    pub(crate) warning: bool,
    pub(crate) span: Option<Span>,
}

impl RuntimeError {
    pub(crate) fn condition_type_name(&self) -> String {
        match self {
            Self::Read(_) => "READER-ERROR".to_owned(),
            Self::Compile(_) => "COMPILER-ERROR".to_owned(),
            Self::UnboundVariable { .. } => "UNBOUND-VARIABLE".to_owned(),
            Self::NotCallable { .. } | Self::Type { .. } => "TYPE-ERROR".to_owned(),
            Self::Arity { .. } => "PROGRAM-ERROR".to_owned(),
            Self::InvalidForm { .. } => "SIMPLE-ERROR".to_owned(),
            Self::Signaled {
                condition, warning, ..
            } => {
                if *warning {
                    "SIMPLE-WARNING".to_owned()
                } else {
                    signaled.condition.clone()
                }
            }
            Self::Package { .. } => "PACKAGE-ERROR".to_owned(),
            Self::ReturnFrom { .. }
            | Self::Go { .. }
            | Self::Throw { .. }
            | Self::InvokeRestart { .. } => "CONTROL-ERROR".to_owned(),
            Self::DivisionByZero => "DIVISION-BY-ZERO".to_owned(),
            Self::NumericOverflow => "ARITHMETIC-ERROR".to_owned(),
            Self::Io(_) => "FILE-ERROR".to_owned(),
        }
    }

    pub(crate) fn matches_condition(&self, condition: &str) -> bool {
        if matches!(
            self,
            Self::ReturnFrom { .. }
                | Self::Go { .. }
                | Self::Throw { .. }
                | Self::InvokeRestart { .. }
        ) {
            return false;
        }

        let condition = normalize_condition_name(condition);
        if matches!(
            condition.as_str(),
            "CONDITION" | "ERROR" | "SERIOUS-CONDITION"
        ) {
            return match self {
                Self::Signaled(signaled) => {
                    if condition == "CONDITION" {
                        true
                    } else if signaled.warning {
                        false
                    } else {
                        condition_types
                            .iter()
                            .any(|type_name| normalize_condition_name(type_name) == condition)
                            || matches!(
                                normalize_condition_name(signaled).as_str(),
                                "SIMPLE-ERROR"
                                    | "DIVISION-BY-ZERO"
                                    | "ARITHMETIC-ERROR"
                                    | "TYPE-ERROR"
                                    | "PROGRAM-ERROR"
                                    | "PACKAGE-ERROR"
                                    | "READER-ERROR"
                                    | "COMPILER-ERROR"
                                    | "FILE-ERROR"
                                    | "UNBOUND-VARIABLE"
                            )
                    }
                }
                _ => true,
            };
        }

        match self {
            Self::Signaled(signaled) => {
                condition == normalize_condition_name(&signaled.condition)
                    || signaled
                        .condition_types
                        .iter()
                        .any(|type_name| normalize_condition_name(type_name) == condition)
                    || (signaled.warning && condition == "WARNING")
                    || (!signaled.warning && condition == "SIMPLE-CONDITION")
            }
            Self::DivisionByZero => {
                matches!(condition.as_str(), "DIVISION-BY-ZERO" | "ARITHMETIC-ERROR")
            }
            Self::NumericOverflow => condition == "ARITHMETIC-ERROR",
            _ => condition == self.condition_type_name(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => error.fmt(formatter),
            Self::Compile(error) => error.fmt(formatter),
            Self::UnboundVariable { name, span } => {
                write!(formatter, "unbound variable {name}")?;
                write_span(formatter, *span)
            }
            Self::NotCallable { value, span } => {
                write!(formatter, "{value} is not callable")?;
                write_span(formatter, *span)
            }
            Self::Arity {
                function,
                expected,
                actual,
            } => write!(
                formatter,
                "{function} expected {expected} arguments, received {actual}"
            ),
            Self::Type {
                expected,
                actual,
                span,
            } => {
                write!(formatter, "expected {expected}, received {actual}")?;
                write_span(formatter, *span)
            }
            Self::InvalidForm { message, span } => {
                formatter.write_str(message)?;
                write_span(formatter, *span)
            }
            Self::Signaled(signaled) => {
                formatter.write_str(&signaled.message)?;
                write_span(formatter, signaled.span)
            }
            Self::Package { message, span } => {
                formatter.write_str(message)?;
                write_span(formatter, *span)
            }
            Self::ReturnFrom { block, span, .. } => {
                write!(formatter, "return-from {block}")?;
                write_span(formatter, *span)
            }
            Self::Go { tag, span, .. } => {
                write!(formatter, "go {tag}")?;
                write_span(formatter, *span)
            }
            Self::Throw { tag, span, .. } => {
                write!(formatter, "throw {tag}")?;
                write_span(formatter, *span)
            }
            Self::InvokeRestart { name, span, .. } => {
                write!(formatter, "invoke-restart {name}")?;
                write_span(formatter, *span)
            }
            Self::DivisionByZero => formatter.write_str("division by zero"),
            Self::NumericOverflow => formatter.write_str("numeric overflow"),
            Self::Io(message) => formatter.write_str(message),
        }
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Compile(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ReadError> for RuntimeError {
    fn from(error: ReadError) -> Self {
        Self::Read(error)
    }
}

impl From<CompileError> for RuntimeError {
    fn from(error: CompileError) -> Self {
        Self::Compile(error)
    }
}

fn write_span(formatter: &mut fmt::Formatter<'_>, span: Option<Span>) -> fmt::Result {
    if let Some(span) = span {
        write!(formatter, " at byte {}..{}", span.start, span.end)?;
    }
    Ok(())
}

fn normalize_condition_name(condition: &str) -> String {
    condition.trim_start_matches(':').to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_value_wrappers_preserve_their_comparison_semantics() {
        let returned = ReturnValue::new(Value::list(vec![Value::Integer(1)]));
        assert_eq!(
            returned,
            ReturnValue::new(Value::list(vec![Value::Integer(1)]))
        );
        assert_eq!(returned.into_value().to_string(), "(1)");

        let tag = ThrowTag::new(Value::symbol_exact("Tag"));
        assert_eq!(tag, ThrowTag::new(Value::symbol_exact("Tag")));
        assert_ne!(tag, ThrowTag::new(Value::symbol_exact("Other")));
        assert_eq!(tag.to_string(), "|Tag|");
    }

    fn signaled(warning: bool) -> RuntimeError {
        RuntimeError::Signaled(Box::new(SignaledError {
            condition: "custom-condition".to_owned(),
            condition_types: vec!["parent-condition".to_owned()],
            message: "message".to_owned(),
            format_control: None,
            format_arguments: Vec::new(),
            warning,
            span: Some(Span::new(2, 5)),
        }))
    }

    #[test]
    fn condition_matching_normalizes_names_and_handles_control_errors() {
        let error = signaled(false);
        assert!(error.matches_condition(":custom-condition"));
        assert!(error.matches_condition("PARENT-CONDITION"));
        assert!(error.matches_condition("simple-condition"));
        assert!(!error.matches_condition("serious-condition"));
        assert!(!error.matches_condition("warning"));

        let control = RuntimeError::Go {
            tag: "done".to_owned(),
            target: None,
            span: None,
        };
        assert!(!control.matches_condition("condition"));
    }

    #[test]
    fn warning_matching_excludes_error_and_includes_warning() {
        let warning = signaled(true);
        assert!(warning.matches_condition("WARNING"));
        assert!(warning.matches_condition("CONDITION"));
        assert!(!warning.matches_condition("ERROR"));
        assert_eq!(warning.condition_type_name(), "SIMPLE-WARNING");
    }

    #[test]
    fn built_in_error_conditions_are_classified() {
        assert!(RuntimeError::DivisionByZero.matches_condition("arithmetic-error"));
        assert!(!RuntimeError::DivisionByZero.matches_condition("warning"));
        assert!(RuntimeError::NumericOverflow.matches_condition("ARITHMETIC-ERROR"));
        assert_eq!(
            RuntimeError::Io("io".to_owned()).condition_type_name(),
            "FILE-ERROR"
        );
    }

    #[test]
    fn condition_type_names_cover_non_signaled_runtime_errors() {
        let cases = [
            (
                RuntimeError::UnboundVariable {
                    name: "x".to_owned(),
                    span: None,
                },
                "UNBOUND-VARIABLE",
            ),
            (
                RuntimeError::NotCallable {
                    value: "x".to_owned(),
                    span: None,
                },
                "TYPE-ERROR",
            ),
            (
                RuntimeError::Arity {
                    function: "f".to_owned(),
                    expected: "1".to_owned(),
                    actual: 2,
                },
                "PROGRAM-ERROR",
            ),
            (
                RuntimeError::InvalidForm {
                    message: "bad".to_owned(),
                    span: None,
                },
                "SIMPLE-ERROR",
            ),
            (
                RuntimeError::Package {
                    message: "bad package".to_owned(),
                    span: None,
                },
                "PACKAGE-ERROR",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.condition_type_name(), expected);
            assert!(error.matches_condition(expected));
            assert!(error.matches_condition("condition"));
            assert!(error.matches_condition("error"));
        }
    }

    #[test]
    fn display_includes_spans_and_control_details() {
        let error = RuntimeError::UnboundVariable {
            name: "x".to_owned(),
            span: Some(Span::new(3, 4)),
        };
        assert_eq!(error.to_string(), "unbound variable x at byte 3..4");
        assert_eq!(signaled(false).to_string(), "message at byte 2..5");
        assert_eq!(RuntimeError::DivisionByZero.to_string(), "division by zero");
    }

    #[test]
    fn display_uses_human_readable_messages_for_each_control_error() {
        let value = ReturnValue::new(Value::Integer(1));
        let cases = [
            (
                RuntimeError::NotCallable {
                    value: "1".to_owned(),
                    span: None,
                },
                "1 is not callable",
            ),
            (
                RuntimeError::Arity {
                    function: "f".to_owned(),
                    expected: "1".to_owned(),
                    actual: 2,
                },
                "f expected 1 arguments, received 2",
            ),
            (
                RuntimeError::Type {
                    expected: "integer".to_owned(),
                    actual: "string".to_owned(),
                    span: None,
                },
                "expected integer, received string",
            ),
            (
                RuntimeError::Package {
                    message: "missing package".to_owned(),
                    span: None,
                },
                "missing package",
            ),
            (
                RuntimeError::ReturnFrom {
                    block: "done".to_owned(),
                    target: None,
                    value: value.clone(),
                    span: None,
                },
                "return-from done",
            ),
            (
                RuntimeError::Go {
                    tag: "loop".to_owned(),
                    target: None,
                    span: None,
                },
                "go loop",
            ),
            (
                RuntimeError::Throw {
                    tag: ThrowTag::new(Value::symbol("tag")),
                    value: value.clone(),
                    span: None,
                },
                "throw TAG",
            ),
            (
                RuntimeError::InvokeRestart {
                    name: "use-value".to_owned(),
                    value,
                    arguments: Vec::new(),
                    span: None,
                },
                "invoke-restart use-value",
            ),
            (RuntimeError::NumericOverflow, "numeric overflow"),
            (RuntimeError::Io("io failed".to_owned()), "io failed"),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }
}

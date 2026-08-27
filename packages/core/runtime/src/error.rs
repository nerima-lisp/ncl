use std::error::Error;
use std::fmt;

use ncl_compiler::CompileError;
use ncl_syntax::{ReadError, Span};

use crate::Value;

#[derive(Clone, Debug)]
/// A value returned through a non-local control transfer.
pub struct ReturnValue(Box<Value>);

impl ReturnValue {
    /// Wraps a runtime value.
    #[must_use]
    pub fn new(value: Value) -> Self {
        Self(Box::new(value))
    }

    /// Extracts the wrapped runtime value.
    #[must_use]
    pub fn into_value(self) -> Value {
        *self.0
    }
}

impl PartialEq for ReturnValue {
    fn eq(&self, other: &Self) -> bool {
        self.0.equal_value(&other.0)
    }
}

impl Eq for ReturnValue {}

#[derive(Clone, Debug)]
/// A tag used by `catch` and `throw` control transfers.
pub struct ThrowTag(Box<Value>);

impl ThrowTag {
    pub(crate) fn new(value: Value) -> Self {
        Self(Box::new(value))
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
/// Payload for a signaled condition.
pub struct SignaledError {
    pub(crate) condition: String,
    pub(crate) condition_types: Box<[String]>,
    pub(crate) message: String,
    pub(crate) format_control: Option<String>,
    pub(crate) format_arguments: Box<[ReturnValue]>,
    pub(crate) warning: bool,
    pub(crate) span: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// An error produced while reading, compiling, or evaluating NCL code.
pub enum RuntimeError {
    /// A reader error.
    Read(Box<ReadError>),
    /// A compiler error.
    Compile(Box<CompileError>),
    /// A reference to an unbound variable.
    UnboundVariable {
        /// The variable name.
        name: String,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// An attempt to call a non-callable value.
    NotCallable {
        /// A display representation of the value.
        value: String,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// A function was called with an invalid number of arguments.
    Arity {
        /// The function name.
        function: String,
        /// The expected arity description.
        expected: String,
        /// The number of arguments received.
        actual: usize,
    },
    /// An argument had an unexpected type.
    Type {
        /// The expected type.
        expected: String,
        /// The actual type.
        actual: String,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// A form is invalid in its current context.
    InvalidForm {
        /// A human-readable explanation.
        message: String,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// A condition was signaled by the program.
    Signaled(Box<SignaledError>),
    /// A package operation failed.
    Package {
        /// A human-readable explanation.
        message: String,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// A `return-from` transfer escaped to the runtime boundary.
    ReturnFrom {
        /// The block name.
        block: String,
        /// The internal target identifier.
        target: Option<u64>,
        /// The returned value.
        value: ReturnValue,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// A `go` transfer escaped to the runtime boundary.
    Go {
        /// The tag name.
        tag: String,
        /// The internal target identifier.
        target: Option<u64>,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// A `throw` transfer escaped to the runtime boundary.
    Throw {
        /// The thrown tag.
        tag: ThrowTag,
        /// The thrown value.
        value: ReturnValue,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// A restart invocation escaped to the runtime boundary.
    InvokeRestart {
        /// The restart name.
        name: String,
        /// The primary restart value.
        value: ReturnValue,
        /// Additional restart arguments.
        arguments: Vec<ReturnValue>,
        /// The source span, when available.
        span: Option<Span>,
    },
    /// An arithmetic operation attempted to divide by zero.
    DivisionByZero,
    /// An arithmetic operation exceeded its representation limits.
    NumericOverflow,
    /// An I/O operation failed.
    Io(String),
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
            Self::Signaled(error) => {
                if error.warning {
                    "SIMPLE-WARNING".to_owned()
                } else {
                    error.condition.clone()
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
                Self::Signaled(error) => {
                    if condition == "CONDITION" {
                        true
                    } else if error.warning {
                        false
                    } else {
                        error
                            .condition_types
                            .iter()
                            .any(|type_name| normalize_condition_name(type_name) == condition)
                            || matches!(
                                normalize_condition_name(&error.condition).as_str(),
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
            Self::Signaled(error) => {
                condition == normalize_condition_name(&error.condition)
                    || error
                        .condition_types
                        .iter()
                        .any(|type_name| normalize_condition_name(type_name) == condition)
                    || (error.warning && condition == "WARNING")
                    || (!error.warning && condition == "SIMPLE-CONDITION")
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
            Self::InvalidForm { message, span } | Self::Package { message, span } => {
                formatter.write_str(message)?;
                write_span(formatter, *span)
            }
            Self::Signaled(error) => {
                formatter.write_str(&error.message)?;
                write_span(formatter, error.span)
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
        Self::Read(Box::new(error))
    }
}

impl From<CompileError> for RuntimeError {
    fn from(error: CompileError) -> Self {
        Self::Compile(Box::new(error))
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

    fn value() -> Value {
        Value::Integer(7)
    }

    #[test]
    fn return_values_compare_by_lisp_equality_and_round_trip() {
        let returned = ReturnValue::new(value());
        assert_eq!(returned, ReturnValue::new(Value::Integer(7)));
        assert!(returned.into_value().equal_value(&value()));
    }

    #[test]
    fn throw_tags_use_identity_equality_and_display() {
        let tag = ThrowTag::new(Value::symbol("TAG"));
        assert!(tag.matches(&Value::symbol("TAG")));
        assert!(!tag.matches(&Value::symbol("OTHER")));
        assert_eq!(tag.to_string(), "TAG");
    }

    #[test]
    fn condition_matching_normalizes_names_and_handles_control_errors() {
        let warning = RuntimeError::Signaled(Box::new(SignaledError {
            condition: "CUSTOM-WARNING".to_owned(),
            condition_types: vec!["SIMPLE-WARNING".to_owned()].into_boxed_slice(),
            message: "warning".to_owned(),
            format_control: None,
            format_arguments: Box::default(),
            warning: true,
            span: None,
        }));
        assert!(warning.matches_condition(":warning"));
        assert!(warning.matches_condition("condition"));
        assert!(!warning.matches_condition("error"));

        let error = RuntimeError::DivisionByZero;
        assert_eq!(error.condition_type_name(), "DIVISION-BY-ZERO");
        assert!(error.matches_condition("arithmetic-error"));
        assert_eq!(error.to_string(), "division by zero");

        let control = RuntimeError::Go {
            tag: "DONE".to_owned(),
            target: None,
            span: Some(Span::new(2, 5)),
        };
        assert!(!control.matches_condition("control-error"));
        assert_eq!(control.to_string(), "go DONE at byte 2..5");
    }

    #[test]
    fn condition_matching_covers_builtin_condition_hierarchy() {
        let cases = [
            (RuntimeError::NumericOverflow, "arithmetic-error", true),
            (RuntimeError::NumericOverflow, "division-by-zero", false),
            (
                RuntimeError::Type {
                    expected: "integer".to_owned(),
                    actual: "string".to_owned(),
                    span: None,
                },
                "condition",
                true,
            ),
            (
                RuntimeError::Type {
                    expected: "integer".to_owned(),
                    actual: "string".to_owned(),
                    span: None,
                },
                "simple-condition",
                false,
            ),
            (
                RuntimeError::Arity {
                    function: "f".to_owned(),
                    expected: "one".to_owned(),
                    actual: 2,
                },
                "program-error",
                true,
            ),
            (RuntimeError::Io("failed".to_owned()), "file-error", true),
        ];

        for (error, condition, expected) in cases {
            assert_eq!(error.matches_condition(condition), expected, "{condition}");
        }
    }

    #[test]
    fn runtime_error_display_includes_spans_and_variant_messages() {
        let span = Some(Span::new(1, 4));
        let errors = [
            (
                RuntimeError::UnboundVariable {
                    name: "X".to_owned(),
                    span,
                },
                "unbound variable X at byte 1..4",
            ),
            (
                RuntimeError::NotCallable {
                    value: "7".to_owned(),
                    span,
                },
                "7 is not callable at byte 1..4",
            ),
            (
                RuntimeError::Arity {
                    function: "F".to_owned(),
                    expected: "1".to_owned(),
                    actual: 2,
                },
                "F expected 1 arguments, received 2",
            ),
            (
                RuntimeError::Type {
                    expected: "INTEGER".to_owned(),
                    actual: "STRING".to_owned(),
                    span,
                },
                "expected INTEGER, received STRING at byte 1..4",
            ),
            (
                RuntimeError::InvalidForm {
                    message: "bad form".to_owned(),
                    span,
                },
                "bad form at byte 1..4",
            ),
            (
                RuntimeError::Package {
                    message: "bad package".to_owned(),
                    span,
                },
                "bad package at byte 1..4",
            ),
            (RuntimeError::NumericOverflow, "numeric overflow"),
            (RuntimeError::Io("io failed".to_owned()), "io failed"),
        ];
        for (error, expected) in errors {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn runtime_error_display_covers_control_and_signaled_variants() {
        let span = Some(Span::new(3, 6));
        let signaled = RuntimeError::Signaled(Box::new(SignaledError {
            condition: "SIMPLE-ERROR".to_owned(),
            condition_types: Box::default(),
            message: "failed".to_owned(),
            format_control: None,
            format_arguments: Box::default(),
            warning: false,
            span,
        }));
        let cases = [
            (signaled, "failed at byte 3..6"),
            (
                RuntimeError::ReturnFrom {
                    block: "BLOCK".to_owned(),
                    target: None,
                    value: ReturnValue::new(Value::Nil),
                    span,
                },
                "return-from BLOCK at byte 3..6",
            ),
            (
                RuntimeError::Throw {
                    tag: ThrowTag::new(Value::symbol("TAG")),
                    value: ReturnValue::new(Value::Nil),
                    span,
                },
                "throw TAG at byte 3..6",
            ),
            (
                RuntimeError::InvokeRestart {
                    name: "ABORT".to_owned(),
                    value: ReturnValue::new(Value::Nil),
                    arguments: Vec::new(),
                    span,
                },
                "invoke-restart ABORT at byte 3..6",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn condition_type_names_cover_all_runtime_error_categories() {
        let cases = [
            (
                RuntimeError::UnboundVariable {
                    name: "X".into(),
                    span: None,
                },
                "UNBOUND-VARIABLE",
            ),
            (
                RuntimeError::NotCallable {
                    value: "X".into(),
                    span: None,
                },
                "TYPE-ERROR",
            ),
            (
                RuntimeError::Type {
                    expected: "X".into(),
                    actual: "Y".into(),
                    span: None,
                },
                "TYPE-ERROR",
            ),
            (
                RuntimeError::Arity {
                    function: "F".into(),
                    expected: "1".into(),
                    actual: 0,
                },
                "PROGRAM-ERROR",
            ),
            (
                RuntimeError::InvalidForm {
                    message: "bad".into(),
                    span: None,
                },
                "SIMPLE-ERROR",
            ),
            (
                RuntimeError::Package {
                    message: "bad".into(),
                    span: None,
                },
                "PACKAGE-ERROR",
            ),
            (
                RuntimeError::ReturnFrom {
                    block: "B".into(),
                    target: None,
                    value: ReturnValue::new(Value::Nil),
                    span: None,
                },
                "CONTROL-ERROR",
            ),
            (
                RuntimeError::Throw {
                    tag: ThrowTag::new(Value::Nil),
                    value: ReturnValue::new(Value::Nil),
                    span: None,
                },
                "CONTROL-ERROR",
            ),
            (
                RuntimeError::InvokeRestart {
                    name: "A".into(),
                    value: ReturnValue::new(Value::Nil),
                    arguments: Vec::new(),
                    span: None,
                },
                "CONTROL-ERROR",
            ),
            (RuntimeError::NumericOverflow, "ARITHMETIC-ERROR"),
            (RuntimeError::Io("io".into()), "FILE-ERROR"),
        ];
        for (error, expected) in cases {
            assert_eq!(error.condition_type_name(), expected);
        }
    }
}

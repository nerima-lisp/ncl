use std::error::Error;
use std::fmt;

use ncl_compiler::CompileError;
use ncl_syntax::{ReadError, Span};

use crate::Value;

#[derive(Clone, Debug)]
pub struct ReturnValue(Value);

impl ReturnValue {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

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
    Signaled {
        condition: String,
        condition_types: Vec<String>,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<ReturnValue>,
        warning: bool,
        span: Option<Span>,
    },
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
                    condition.clone()
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
                Self::Signaled {
                    condition: signaled,
                    condition_types,
                    warning,
                    ..
                } => {
                    if condition == "CONDITION" {
                        true
                    } else if *warning {
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
            Self::Signaled {
                condition: signaled,
                condition_types,
                warning,
                ..
            } => {
                condition == normalize_condition_name(signaled)
                    || condition_types
                        .iter()
                        .any(|type_name| normalize_condition_name(type_name) == condition)
                    || (*warning && condition == "WARNING")
                    || (!*warning && condition == "SIMPLE-CONDITION")
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
            Self::Signaled { message, span, .. } => {
                formatter.write_str(message)?;
                write_span(formatter, *span)
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

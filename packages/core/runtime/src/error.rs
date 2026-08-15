use std::error::Error;
use std::fmt;

use ncl_compiler::{CompileError, CompileErrorKind};
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
        match error.kind {
            CompileErrorKind::Arity {
                operator,
                expected,
                actual,
            } if matches!(
                operator.as_str(),
                "PROG1" | "PROG2" | "IF" | "LET" | "LET*" | "FLET" | "LABELS" | "WHEN" | "UNLESS"
            ) =>
            {
                Self::Arity {
                function: operator.to_ascii_lowercase(),
                expected,
                actual,
                }
            }
            CompileErrorKind::ExpectedList { context }
                if matches!(
                    context.as_str(),
                    "DOTIMES binding"
                        | "DOLIST binding"
                        | "DO bindings"
                        | "DO termination"
                        | "DO binding"
                        | "PROG bindings"
                        | "WITH-OPEN-FILE binding"
                        | "WITH-OUTPUT-TO-STRING binding"
                        | "WITH-INPUT-FROM-STRING binding"
                        | "let bindings"
                        | "let binding"
                        | "local function bindings"
                        | "local function binding"
                        | "handler-bind handler list"
                        | "handler-bind clause"
                        | "handler-case clause"
                        | "handler-case variable list"
                        | "restart-bind binding list"
                        | "restart-bind clause"
                        | "WITH-SIMPLE-RESTART restart clause"
                        | "restart-case clause"
                        | "cond clause"
                        | "case clause"
                        | "typecase clause"
                        | "EVAL-WHEN situations"
                        | "MULTIPLE-VALUE-BIND variables"
                        | "MULTIPLE-VALUE-SETQ variables"
                        | "parameters"
                ) =>
            {
                let message = match context.as_str() {
                    "DOTIMES binding" => "dotimes binding must be a list".to_string(),
                    "DOLIST binding" => "dolist binding must be a list".to_string(),
                    "DO bindings" => "do bindings must be a list".to_string(),
                    "DO termination" => "do termination must be a list".to_string(),
                    "DO binding" => "do binding must be a list".to_string(),
                    "PROG bindings" => "prog bindings must be a list".to_string(),
                    "EVAL-WHEN situations" => "eval-when situations must be a list".to_string(),
                    "MULTIPLE-VALUE-BIND variables" => {
                        "multiple-value-bind variables must be a list".to_string()
                    }
                    "MULTIPLE-VALUE-SETQ variables" => {
                        "multiple-value-setq variables must be a list".to_string()
                    }
                    "WITH-OPEN-FILE binding" => {
                        "with-open-file binding must be a list".to_string()
                    }
                    "WITH-OUTPUT-TO-STRING binding" => {
                        "with-output-to-string binding must be a list".to_string()
                    }
                    "WITH-INPUT-FROM-STRING binding" => {
                        "with-input-from-string binding must be a list".to_string()
                    }
                    "WITH-SIMPLE-RESTART restart clause" => {
                        "with-simple-restart restart clause must be a list".to_string()
                    }
                    "cond clause" => "cond clauses must be lists".to_string(),
                    "case clause" => "case clauses must be lists".to_string(),
                    "typecase clause" => "typecase clauses must be lists".to_string(),
                    _ => format!("{context} must be a list"),
                };

                Self::InvalidForm {
                    message,
                    span: Some(error.span),
                }
            }
            CompileErrorKind::ExpectedSymbol { context }
                if matches!(
                    context.as_str(),
                    "GO tag"
                        | "parameter"
                        | "&rest parameter"
                        | "BLOCK name"
                        | "RETURN-FROM name"
                        | "PROG binding name"
                        | "DO binding name"
                        | "DOTIMES variable"
                        | "DOLIST variable"
                        | "handler-case condition"
                        | "handler-bind condition"
                        | "RESTART-BIND restart name"
                        | "WITH-SIMPLE-RESTART name"
                        | "RESTART-CASE restart name"
                        | "WITH-OPEN-FILE stream variable"
                        | "WITH-OUTPUT-TO-STRING stream variable"
                        | "WITH-INPUT-FROM-STRING stream variable"
                        | "EVAL-WHEN situation"
                        | "MULTIPLE-VALUE-BIND variable"
                        | "MULTIPLE-VALUE-SETQ variable"
                        | "destructuring pattern name"
                        | "destructuring supplied-p name"
                        | "destructuring keyword name"
                        | "destructuring keyword parameter name"
                        | "destructuring auxiliary parameter name"
                        | "destructuring whole parameter name"
                        | "destructuring rest parameter name"
                ) =>
            {
                let message = match context.as_str() {
                    "BLOCK name" | "RETURN-FROM name" => {
                        "block name must be a symbol".to_string()
                    }
                    "PROG binding name" => "prog binding name must be a symbol".to_string(),
                    "DO binding name" => "do binding name must be a symbol".to_string(),
                    "DOTIMES variable" => "dotimes binding name must be a symbol".to_string(),
                    "DOLIST variable" => "dolist binding name must be a symbol".to_string(),
                    "handler-case condition" | "handler-bind condition" => {
                        "condition name must be a symbol".to_string()
                    }
                    "RESTART-BIND restart name"
                    | "WITH-SIMPLE-RESTART name"
                    | "RESTART-CASE restart name" => {
                        "restart name must be a symbol".to_string()
                    }
                    "EVAL-WHEN situation" => {
                        "eval-when situations must contain symbols".to_string()
                    }
                    "GO tag" => "go tag must be a symbol or integer".to_string(),
                    "WITH-OPEN-FILE stream variable" => {
                        "with-open-file stream variable must be a symbol".to_string()
                    }
                    "WITH-OUTPUT-TO-STRING stream variable" => {
                        "with-output-to-string stream variable must be a symbol".to_string()
                    }
                    "WITH-INPUT-FROM-STRING stream variable" => {
                        "with-input-from-string stream variable must be a symbol".to_string()
                    }
                    "MULTIPLE-VALUE-BIND variable" => {
                        "multiple-value-bind variable must be a symbol".to_string()
                    }
                    "MULTIPLE-VALUE-SETQ variable" => {
                        "multiple-value-setq variable must be a symbol".to_string()
                    }
                    "destructuring keyword name" => {
                        "destructuring keyword designator must be a symbol".to_string()
                    }
                    _ => format!("{context} must be a symbol"),
                };

                Self::InvalidForm {
                    message,
                    span: Some(error.span),
                }
            }
            CompileErrorKind::InvalidForm { message }
                if message.starts_with("duplicate TAGBODY tag ")
                    || message == "PROG binding needs a name and optional value"
                    || message == "PROG binding names must be unique"
                    || message == "DO termination needs an end test"
                    || message == "DO binding needs a name, optional init, and optional step"
                    || message == "DO binding names must be unique"
                    || message == "DOTIMES binding needs a variable, count, and optional result"
                    || message == "DOLIST binding needs a variable, list, and optional result"
                    || message == "WITH-OPEN-FILE binding needs a stream variable and pathname"
                    || message
                        == "WITH-OUTPUT-TO-STRING binding needs a stream variable and optional string place"
                    || message == "WITH-INPUT-FROM-STRING binding needs a stream variable and string"
                    || message == "WITH-INPUT-FROM-STRING options need keyword/value pairs"
                    || message == "WITH-INPUT-FROM-STRING option must be a keyword"
                    || message == "WITH-INPUT-FROM-STRING :start may appear only once"
                    || message == "WITH-INPUT-FROM-STRING :end may appear only once"
                    || message == "WITH-INPUT-FROM-STRING :index may appear only once"
                    || message == "WITH-INPUT-FROM-STRING option is not supported"
                    || message == "handler-case clause needs a condition and variable list"
                    || message == "handler-bind clause needs a condition and handler"
                    || message == "WITH-SIMPLE-RESTART restart clause needs a name and report format"
                    || message == "restart-bind clause needs a name and function"
                    || message == "restart-case clause needs a name, lambda list, and body"
                    || message == "cond clause cannot be empty"
                    || message == "case clause cannot be empty"
                    || message == "typecase clause cannot be empty"
                    || message == "&rest must be followed by one parameter"
                    || message == "&rest must be followed by &key, &aux, or end of lambda-list"
                    || message == "parameter names must be unique" =>
            {
                let message = if message.starts_with("duplicate TAGBODY tag ") {
                    "tagbody contains duplicate tag".to_string()
                } else if message == "PROG binding needs a name and optional value" {
                    "prog binding needs a name and optional value".to_string()
                } else if message == "PROG binding names must be unique" {
                    "prog binding names must be unique".to_string()
                } else if message == "DO termination needs an end test" {
                    "do termination needs an end test".to_string()
                } else if message == "DO binding needs a name, optional init, and optional step" {
                    "do binding needs a name, optional init, and optional step".to_string()
                } else if message == "DO binding names must be unique" {
                    "do binding names must be unique".to_string()
                } else if message
                    == "DOTIMES binding needs a variable, count, and optional result"
                {
                    "dotimes binding needs a name, count, and optional result".to_string()
                } else if message
                    == "DOLIST binding needs a variable, list, and optional result"
                {
                    "dolist binding needs a name, list, and optional result".to_string()
                } else if message
                    == "WITH-OPEN-FILE binding needs a stream variable and pathname"
                {
                    "with-open-file binding needs a stream variable and pathname".to_string()
                } else if message
                    == "WITH-OUTPUT-TO-STRING binding needs a stream variable and optional string place"
                {
                    "with-output-to-string binding needs a stream variable and optional string place"
                        .to_string()
                } else if message
                    == "WITH-INPUT-FROM-STRING binding needs a stream variable and string"
                {
                    "with-input-from-string binding needs a stream variable and string"
                        .to_string()
                } else if message
                    == "WITH-INPUT-FROM-STRING options need keyword/value pairs"
                {
                    "with-input-from-string options need keyword/value pairs".to_string()
                } else if message == "WITH-INPUT-FROM-STRING option must be a keyword" {
                    "with-input-from-string option must be a keyword".to_string()
                } else if message == "WITH-INPUT-FROM-STRING :start may appear only once" {
                    "with-input-from-string :start may appear only once".to_string()
                } else if message == "WITH-INPUT-FROM-STRING :end may appear only once" {
                    "with-input-from-string :end may appear only once".to_string()
                } else if message == "WITH-INPUT-FROM-STRING :index may appear only once" {
                    "with-input-from-string :index may appear only once".to_string()
                } else if message == "WITH-INPUT-FROM-STRING option is not supported" {
                    "with-input-from-string option is not supported".to_string()
                } else if message == "handler-case clause needs a condition and variable list" {
                    "handler-case clause needs a condition and body".to_string()
                } else if message == "handler-bind clause needs a condition and handler" {
                    "handler-bind clause needs a condition and function".to_string()
                } else if message
                    == "WITH-SIMPLE-RESTART restart clause needs a name and report format"
                {
                    "with-simple-restart restart clause needs a name and report format"
                        .to_string()
                } else {
                    message
                };

                Self::InvalidForm {
                    message,
                    span: Some(error.span),
                }
            }
            kind => Self::Compile(CompileError {
                kind,
                span: error.span,
            }),
        }
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

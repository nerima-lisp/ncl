use std::error::Error;

use ncl_compiler::CompileError;
use ncl_syntax::{ReadError, Span};

mod condition;
mod control;
mod display;
mod signaled;

pub use control::{ReturnValue, ThrowTag};
pub use signaled::SignaledError;

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

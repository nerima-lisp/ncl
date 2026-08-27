//! Command-line application and public NCL façade.

/// Command-line argument parsing and process execution.
pub mod cli;

pub use ncl_runtime::{Environment, Function, Runtime, RuntimeError, Value};
pub use ncl_syntax::{Form, FormKind, ReadError, ReadErrorKind, Span, read};

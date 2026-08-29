//! Evaluation runtime and standard primitives for NCL.

pub(crate) mod builtins;
pub(crate) mod environment;
pub(crate) mod error;
pub(crate) mod evaluator;
pub(crate) mod package;
pub(crate) mod value;
pub(crate) mod vm;

pub use environment::Environment;
pub use error::{ReturnValue, RuntimeError};
pub use evaluator::Runtime;
pub(crate) use value::ClosureOptions;
pub use value::{Function, Rational, Stream, Value};

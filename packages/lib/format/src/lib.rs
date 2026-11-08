//! Runtime support for Common Lisp FORMAT controls.
#![expect(
    missing_docs,
    reason = "the public FORMAT AST mirrors the Common Lisp directive grammar consumed by Lisp FORMAT builtins"
)]

mod control;
mod dispatch;
mod executor;
mod model;
mod numeric;
mod parser;
mod pretty;
mod registration;
mod text;

pub use executor::{FormatError, FormatErrorKind, execute};
pub use model::{
    ControlPart, ControlSegment, Directive, DirectiveBody, DirectiveKind, DirectiveModifiers,
    FormatControl, Parameter, Terminator, TerminatorKind,
};
pub use parser::{ParseError, ParseErrorKind, parse};
pub use registration::register;

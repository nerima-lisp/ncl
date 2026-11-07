//! Runtime support for Common Lisp FORMAT controls.
#![allow(missing_docs)]

mod control;
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

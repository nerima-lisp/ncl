//! The Lisp printer: print variables and object output.
//!
//! `ncl-printer` renders `ncl-object` values as Lisp text. The entry points are
//! [`write()`] and [`write_to_string`]; output goes to any [`CharSink`] so
//! `ncl-lib-streams` can adapt its streams later. [`register`] installs the
//! symbols the ownership table assigns to this crate.

mod array;
mod builtins;
mod cons;
mod error;
mod number;
mod opaque;
mod options;
mod print;
mod sink;
mod string;
mod symbol;

pub use builtins::register;
pub use error::PrintError;
pub use options::{PrintCase, PrintOptions};
pub use print::{write, write_to_string};
pub use sink::{CharSink, StringSink};

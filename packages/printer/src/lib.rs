//! The Lisp printer: print variables and object output.
//!
//! `ncl-printer` renders `ncl-object` values as Lisp text. The entry points are
//! [`write()`] and [`write_to_string`]; output goes to any [`CharSink`] so
//! `ncl-lib-streams` can adapt its streams later. [`register`] installs the
//! symbols the ownership table assigns to this crate, and
//! [`pprint_dispatch`] / [`set_pprint_dispatch`] / [`copy_pprint_dispatch`]
//! operate on the `*print-pprint-dispatch*` table.

mod array;
mod builtins;
mod circle;
mod cons;
mod error;
mod number;
mod opaque;
mod options;
mod print;
mod sink;
mod string;
mod symbol;

pub use builtins::{copy_pprint_dispatch, pprint_dispatch, register, set_pprint_dispatch};
pub use error::PrintError;
pub use options::{
    ArrayMode, CircleMode, CircleSharingMode, EscapeMode, GensymMode, NonNegative, PrettyMode,
    PrintBase, PrintCase, PrintOptions, RadixMode, ReadabilityMode,
};
pub use print::{write, write_to_string};
pub use sink::{CharSink, StringSink};

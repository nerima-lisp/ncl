//! Integer bit operations split by operation family.

pub(crate) use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

mod boole;
mod fields;
mod logic;

pub use boole::*;
pub(crate) use fields::*;
pub(crate) use logic::*;

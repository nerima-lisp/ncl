//! Dependency-free intermediate representation shared by the compiler lanes.
#![allow(clippy::all)]

mod builder;
mod fixnum;
mod text;
mod types;
mod verify;

pub use builder::FunctionBuilder;
pub use fixnum::*;
pub use text::{ParseError, parse};
pub use types::*;
pub use verify::{AddressLivenessBoundary, VerifyError, proven_fixnum_values, verify};

#[cfg(test)]
mod tests;

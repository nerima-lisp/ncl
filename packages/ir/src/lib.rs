//! Dependency-free intermediate representation shared by the compiler lanes.
#![allow(clippy::all)]

mod builder;
mod text;
mod types;
mod verify;

pub use builder::FunctionBuilder;
pub use text::{ParseError, parse};
pub use types::*;
pub use verify::{VerifyError, verify};

#[cfg(test)]
mod tests;

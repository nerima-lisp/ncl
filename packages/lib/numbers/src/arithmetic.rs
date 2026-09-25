//! Numeric builtins split by arithmetic responsibility.

pub(crate) use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

mod basic;
mod comparison;
mod core;
mod dispatch;
mod number_theory;
mod predicates;
mod rounding;

pub use basic::*;
pub use comparison::*;
pub(crate) use core::*;
pub use dispatch::*;
pub use number_theory::*;
pub use predicates::*;
pub use rounding::*;

#[cfg(test)]
mod tests {
    include!("arithmetic/test_cases.inc");
}

//! Numeric builtins split by arithmetic responsibility.

use ncl_object::{BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word};
use std::cmp::Ordering;

mod basic;
mod comparison;
mod core;
mod dispatch;
mod number_theory;
mod predicates;
mod rounding;

pub use basic::*;
pub use comparison::*;
use core::{
    Number, add_pair, args_numbers, bool_word, div_pair, gcd_i128, integer, mul_pair, number,
    ratio, sub_pair, word,
};
pub use dispatch::*;
pub use number_theory::*;
pub use predicates::*;
pub use rounding::*;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::needless_borrow
)]
mod tests {
    include!("arithmetic/test_cases.inc");
}

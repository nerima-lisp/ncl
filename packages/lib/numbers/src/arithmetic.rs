//! Numeric builtins split by arithmetic responsibility.

use ncl_object::{BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word};
use std::cmp::Ordering;

mod basic;
mod comparison;
mod core;
mod predicates;

pub use basic::*;
pub use comparison::*;
use core::{
    add_pair, args_numbers, bool_word, div_pair, integer, mul_pair, number, ratio, sub_pair, word,
    Number,
};
pub use predicates::*;

macro_rules! typed_runtime {
    ($name:ident, $function:ident) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            _: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $function(ctx, runtime, args.as_slice())
        }
    };
}

macro_rules! typed_predicate {
    ($name:ident, $function:ident) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            _: &Runtime,
            args: &BuiltinArgs<'_>,
            _: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $function(ctx, args.as_slice())
        }
    };
}

typed_predicate!(typed_numberp, numberp);
typed_predicate!(typed_integerp, integerp);
typed_predicate!(typed_rationalp, rationalp);
typed_predicate!(typed_floatp, floatp);
typed_predicate!(typed_realp, realp);
typed_predicate!(typed_complexp, complexp);
typed_predicate!(typed_zerop, zerop);
typed_predicate!(typed_plusp, plusp);
typed_predicate!(typed_minusp, minusp);
typed_predicate!(typed_evenp, evenp);
typed_predicate!(typed_oddp, oddp);

typed_runtime!(typed_add, add);
typed_runtime!(typed_sub, sub);
typed_runtime!(typed_mul, mul);
typed_runtime!(typed_div, div);
typed_runtime!(typed_equal, equal);
typed_runtime!(typed_eq, eq);
typed_runtime!(typed_eql, eql);
typed_runtime!(typed_not_equal, not_equal);
typed_runtime!(typed_less, less);
typed_runtime!(typed_greater, greater);
typed_runtime!(typed_less_equal, less_equal);
typed_runtime!(typed_greater_equal, greater_equal);
typed_runtime!(typed_max, max);
typed_runtime!(typed_min, min);
typed_runtime!(typed_one_plus, one_plus);
typed_runtime!(typed_one_minus, one_minus);
typed_runtime!(typed_abs, abs);
typed_runtime!(typed_signum, signum);

#[cfg(test)]
#[allow(clippy::needless_borrow)]
mod tests {
    include!("arithmetic/test_cases.inc");
}

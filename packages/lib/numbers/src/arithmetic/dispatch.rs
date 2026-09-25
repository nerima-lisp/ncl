#![allow(clippy::needless_pass_by_ref_mut)]

use super::{
    abs, add, ceiling, complexp, div, equal, evenp, fceiling, ffloor, floatp, floor, fround,
    ftruncate, gcd, greater, greater_equal, integerp, isqrt, lcm, less, less_equal, max, min,
    minusp, modulo, mul, not_equal, numberp, oddp, one_minus, one_plus, plusp, rationalp, realp,
    remainder, round, signum, sub, truncate, zerop, BuiltinArgs, MultipleValues, ObjectError,
    Runtime, ThreadContext, Word,
};

pub fn dispatch_add(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    add(ctx, runtime, args)
}
pub fn dispatch_sub(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    sub(ctx, runtime, args)
}
pub fn dispatch_mul(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    mul(ctx, runtime, args)
}
pub fn dispatch_div(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    div(ctx, runtime, args)
}
pub fn dispatch_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    equal(ctx, runtime, args)
}
pub fn dispatch_floor(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    floor(ctx, runtime, args, values)
}

macro_rules! predicate_dispatch {
    ($name:ident, $predicate:ident) => {
        pub fn $name(
            _: &Runtime,
            ctx: &ThreadContext,
            args: &[Word],
            _: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $predicate(ctx, args)
        }
    };
}
predicate_dispatch!(dispatch_numberp, numberp);
predicate_dispatch!(dispatch_integerp, integerp);
predicate_dispatch!(dispatch_rationalp, rationalp);
predicate_dispatch!(dispatch_floatp, floatp);
predicate_dispatch!(dispatch_realp, realp);
predicate_dispatch!(dispatch_complexp, complexp);

macro_rules! runtime_dispatch {
    ($name:ident, $function:ident) => {
        pub fn $name(
            runtime: &Runtime,
            ctx: &mut ThreadContext,
            args: &[Word],
            _: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $function(ctx, runtime, args)
        }
    };
}
runtime_dispatch!(dispatch_one_plus, one_plus);
runtime_dispatch!(dispatch_one_minus, one_minus);
runtime_dispatch!(dispatch_abs, abs);
runtime_dispatch!(dispatch_signum, signum);
runtime_dispatch!(dispatch_max, max);
runtime_dispatch!(dispatch_min, min);
runtime_dispatch!(dispatch_mod, modulo);
runtime_dispatch!(dispatch_rem, remainder);
runtime_dispatch!(dispatch_gcd, gcd);
runtime_dispatch!(dispatch_lcm, lcm);
runtime_dispatch!(dispatch_isqrt, isqrt);

macro_rules! values_dispatch {
    ($name:ident, $function:ident) => {
        pub fn $name(
            runtime: &Runtime,
            ctx: &mut ThreadContext,
            args: &[Word],
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $function(ctx, runtime, args, values)
        }
    };
}
values_dispatch!(dispatch_ceiling, ceiling);
values_dispatch!(dispatch_truncate, truncate);
values_dispatch!(dispatch_round, round);
values_dispatch!(dispatch_ffloor, ffloor);
values_dispatch!(dispatch_fceiling, fceiling);
values_dispatch!(dispatch_ftruncate, ftruncate);
values_dispatch!(dispatch_fround, fround);

pub fn dispatch_not_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    not_equal(ctx, runtime, args)
}
pub fn dispatch_less(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    less(ctx, runtime, args)
}
pub fn dispatch_greater(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    greater(ctx, runtime, args)
}
pub fn dispatch_less_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    less_equal(ctx, runtime, args)
}
pub fn dispatch_greater_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    greater_equal(ctx, runtime, args)
}

macro_rules! typed_legacy_dispatch {
    ($name:ident, $legacy:ident) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $legacy(runtime, ctx, args.as_slice(), values)
        }
    };
}

typed_legacy_dispatch!(typed_dispatch_numberp, dispatch_numberp);
typed_legacy_dispatch!(typed_dispatch_integerp, dispatch_integerp);
typed_legacy_dispatch!(typed_dispatch_rationalp, dispatch_rationalp);
typed_legacy_dispatch!(typed_dispatch_floatp, dispatch_floatp);
typed_legacy_dispatch!(typed_dispatch_realp, dispatch_realp);
typed_legacy_dispatch!(typed_dispatch_complexp, dispatch_complexp);
pub fn typed_dispatch_zerop(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    zerop(ctx, args.as_slice())
}
pub fn typed_dispatch_plusp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    plusp(ctx, args.as_slice())
}
pub fn typed_dispatch_minusp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    minusp(ctx, args.as_slice())
}
pub fn typed_dispatch_evenp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    evenp(ctx, args.as_slice())
}
pub fn typed_dispatch_oddp(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    oddp(ctx, args.as_slice())
}
typed_legacy_dispatch!(typed_dispatch_add, dispatch_add);
typed_legacy_dispatch!(typed_dispatch_sub, dispatch_sub);
typed_legacy_dispatch!(typed_dispatch_mul, dispatch_mul);
typed_legacy_dispatch!(typed_dispatch_div, dispatch_div);
typed_legacy_dispatch!(typed_dispatch_equal, dispatch_equal);
typed_legacy_dispatch!(typed_dispatch_not_equal, dispatch_not_equal);
typed_legacy_dispatch!(typed_dispatch_less, dispatch_less);
typed_legacy_dispatch!(typed_dispatch_greater, dispatch_greater);
typed_legacy_dispatch!(typed_dispatch_less_equal, dispatch_less_equal);
typed_legacy_dispatch!(typed_dispatch_greater_equal, dispatch_greater_equal);
typed_legacy_dispatch!(typed_dispatch_max, dispatch_max);
typed_legacy_dispatch!(typed_dispatch_min, dispatch_min);
typed_legacy_dispatch!(typed_dispatch_one_plus, dispatch_one_plus);
typed_legacy_dispatch!(typed_dispatch_one_minus, dispatch_one_minus);
typed_legacy_dispatch!(typed_dispatch_abs, dispatch_abs);
typed_legacy_dispatch!(typed_dispatch_signum, dispatch_signum);
typed_legacy_dispatch!(typed_dispatch_floor, dispatch_floor);
typed_legacy_dispatch!(typed_dispatch_ceiling, dispatch_ceiling);
typed_legacy_dispatch!(typed_dispatch_truncate, dispatch_truncate);
typed_legacy_dispatch!(typed_dispatch_round, dispatch_round);
typed_legacy_dispatch!(typed_dispatch_ffloor, dispatch_ffloor);
typed_legacy_dispatch!(typed_dispatch_fceiling, dispatch_fceiling);
typed_legacy_dispatch!(typed_dispatch_ftruncate, dispatch_ftruncate);
typed_legacy_dispatch!(typed_dispatch_fround, dispatch_fround);
typed_legacy_dispatch!(typed_dispatch_mod, dispatch_mod);
typed_legacy_dispatch!(typed_dispatch_rem, dispatch_rem);
typed_legacy_dispatch!(typed_dispatch_gcd, dispatch_gcd);
typed_legacy_dispatch!(typed_dispatch_lcm, dispatch_lcm);
typed_legacy_dispatch!(typed_dispatch_isqrt, dispatch_isqrt);

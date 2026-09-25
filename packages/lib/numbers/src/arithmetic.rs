//! Numeric predicates and the core arithmetic callbacks.

mod dispatch;
mod ops;
mod predicates;
mod rounding;
mod value;

#[allow(unused_imports)]
pub use dispatch::{
    dispatch_abs, dispatch_add, dispatch_ceiling, dispatch_complexp, dispatch_div, dispatch_equal,
    dispatch_fceiling, dispatch_ffloor, dispatch_floatp, dispatch_floor, dispatch_fround,
    dispatch_ftruncate, dispatch_gcd, dispatch_greater, dispatch_greater_equal, dispatch_integerp,
    dispatch_isqrt, dispatch_lcm, dispatch_less, dispatch_less_equal, dispatch_max, dispatch_min,
    dispatch_mod, dispatch_mul, dispatch_not_equal, dispatch_numberp, dispatch_one_minus,
    dispatch_one_plus, dispatch_rationalp, dispatch_realp, dispatch_rem, dispatch_round,
    dispatch_signum, dispatch_sub, dispatch_truncate, typed_dispatch_abs, typed_dispatch_add,
    typed_dispatch_ceiling, typed_dispatch_complexp, typed_dispatch_div, typed_dispatch_equal,
    typed_dispatch_evenp, typed_dispatch_fceiling, typed_dispatch_ffloor, typed_dispatch_floatp,
    typed_dispatch_floor, typed_dispatch_fround, typed_dispatch_ftruncate, typed_dispatch_gcd,
    typed_dispatch_greater, typed_dispatch_greater_equal, typed_dispatch_integerp,
    typed_dispatch_isqrt, typed_dispatch_lcm, typed_dispatch_less, typed_dispatch_less_equal,
    typed_dispatch_max, typed_dispatch_min, typed_dispatch_minusp, typed_dispatch_mod,
    typed_dispatch_mul, typed_dispatch_not_equal, typed_dispatch_numberp, typed_dispatch_oddp,
    typed_dispatch_one_minus, typed_dispatch_one_plus, typed_dispatch_plusp,
    typed_dispatch_rationalp, typed_dispatch_realp, typed_dispatch_rem, typed_dispatch_round,
    typed_dispatch_signum, typed_dispatch_sub, typed_dispatch_truncate, typed_dispatch_zerop,
};
#[allow(unused_imports)]
pub use ops::{
    abs, add, div, equal, greater, greater_equal, less, less_equal, max, min, mul, not_equal,
    one_minus, one_plus, signum, sub,
};
#[allow(unused_imports)]
pub use predicates::{
    complexp, evenp, floatp, integerp, minusp, numberp, oddp, plusp, rationalp, realp, zerop,
};
#[allow(unused_imports)]
pub use rounding::{
    ceiling, fceiling, ffloor, floor, fround, ftruncate, gcd, isqrt, lcm, modulo, remainder, round,
    round_dispatch, truncate,
};
#[allow(unused_imports)]
pub use value::integer;

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::unwrap_used
)]
mod tests {
    use super::*;
    use ncl_object::{Runtime, ThreadContext, Word, make_bignum_from_i128};

    fn context() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap();
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).unwrap();
        super::super::register(&runtime).unwrap();
        (runtime, ctx)
    }

    fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
        let function = runtime
            .function(ctx, "COMMON-LISP", name)
            .and_then(|word| ncl_object::FunctionObject::try_from(word).ok())
            .unwrap();
        runtime.call_builtin(ctx, function, args).unwrap()
    }

    fn bignum(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Word {
        make_bignum_from_i128(ctx, runtime, value).unwrap().into()
    }

    #[test]
    fn arithmetic_preserves_fixnum_bignum_boundary_and_argument_order() {
        let (runtime, mut ctx) = context();
        let max = i128::from(i64::MAX >> 4);
        let fixnum = Word::fixnum(max as i64);
        let next = bignum(&mut ctx, &runtime, max + 1);

        let result = add(&mut ctx, &runtime, &[fixnum, Word::fixnum(1)]).unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), max + 1);
        let result = sub(&mut ctx, &runtime, &[next, Word::fixnum(1)]).unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), max);
        let result = mul(
            &mut ctx,
            &runtime,
            &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
        )
        .unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), 24);
        let result = sub(
            &mut ctx,
            &runtime,
            &[Word::fixnum(10), Word::fixnum(2), Word::fixnum(3)],
        )
        .unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), 5);
        let result = one_plus(&mut ctx, &runtime, &[fixnum]).unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), max + 1);
        let result = one_minus(&mut ctx, &runtime, &[next]).unwrap();
        assert_eq!(integer(&ctx, result).unwrap(), max);
    }

    #[test]
    fn comparisons_keep_pairwise_and_chain_semantics_at_boundary() {
        let (runtime, mut ctx) = context();
        let max = i128::from(i64::MAX >> 4);
        let fixnum = Word::fixnum(max as i64);
        let next = bignum(&mut ctx, &runtime, max + 1);
        let after = bignum(&mut ctx, &runtime, max + 2);

        assert_eq!(
            equal(&mut ctx, &runtime, &[fixnum, next]).unwrap(),
            Word::NIL
        );
        assert_eq!(
            not_equal(&mut ctx, &runtime, &[fixnum, next, after]).unwrap(),
            Word::TRUE
        );
        assert_eq!(
            less(&mut ctx, &runtime, &[fixnum, next, after]).unwrap(),
            Word::TRUE
        );
        assert_eq!(
            greater(&mut ctx, &runtime, &[after, next, fixnum]).unwrap(),
            Word::TRUE
        );
        assert_eq!(
            less_equal(&mut ctx, &runtime, &[fixnum, next, next]).unwrap(),
            Word::TRUE
        );
        assert_eq!(
            greater_equal(&mut ctx, &runtime, &[after, next, next]).unwrap(),
            Word::TRUE
        );
    }

    #[test]
    fn registered_arithmetic_and_comparisons_use_builtin_boundary() {
        let (runtime, mut ctx) = context();
        let max = i128::from(i64::MAX >> 4);
        let fixnum = Word::fixnum(max as i64);
        let next = bignum(&mut ctx, &runtime, max + 1);

        let addition = call(&runtime, &mut ctx, "+", &[fixnum, Word::fixnum(1)]);
        assert_eq!(integer(&ctx, addition).unwrap(), max + 1);
        let result = call(&runtime, &mut ctx, "-", &[next, Word::fixnum(1)]);
        assert_eq!(integer(&ctx, result).unwrap(), max);
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                "*",
                &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
            ),
            Word::fixnum(24)
        );
        let result = call(&runtime, &mut ctx, "1+", &[fixnum]);
        assert_eq!(integer(&ctx, result).unwrap(), max + 1);
        let result = call(&runtime, &mut ctx, "1-", &[next]);
        assert_eq!(integer(&ctx, result).unwrap(), max);
        assert_eq!(call(&runtime, &mut ctx, "=", &[fixnum, next]), Word::NIL);
        assert_eq!(call(&runtime, &mut ctx, "/=", &[fixnum, next]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "<", &[fixnum, next]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, ">", &[next, fixnum]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, "<=", &[fixnum, next]), Word::TRUE);
        assert_eq!(call(&runtime, &mut ctx, ">=", &[next, fixnum]), Word::TRUE);
    }
}

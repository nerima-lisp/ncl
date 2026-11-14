use super::number::gcd_i128;
use super::*;
use ncl_object::make_bignum_from_i128;

#[expect(
    clippy::expect_used,
    reason = "fixture failures should fail their test directly without propagating Result branches"
)]
fn test_value<T>(result: Result<T, ObjectError>) -> T {
    result.expect("arithmetic test fixture operation should succeed")
}

fn context() -> (Runtime, ThreadContext) {
    let runtime = test_value(Runtime::new());
    let mut ctx = ThreadContext::new();
    test_value(ctx.register(&runtime));
    test_value(super::super::register(&runtime));
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = test_value(
        runtime
            .function(ctx, "COMMON-LISP", name)
            .ok_or(ObjectError::TypeError),
    );
    let function = test_value(ncl_object::FunctionObject::try_from(function));
    test_value(runtime.call_builtin(ctx, function, args))
}

fn bignum(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Word {
    test_value(make_bignum_from_i128(ctx, runtime, value)).into()
}

fn assert_integer_number(actual: Number, expected: &Integer) {
    let actual = match actual {
        Number::Integer(value) => Integer::from_i128(value),
        Number::BigInteger(value) => value,
        other => panic!("expected integer result, got {other:?}"),
    };
    assert_eq!(&actual, expected);
}

#[test]
fn arithmetic_preserves_fixnum_bignum_boundary_and_argument_order() {
    let (runtime, mut ctx) = context();
    let max = i128::from(crate::MOST_POSITIVE_FIXNUM);
    let fixnum = Word::fixnum(crate::MOST_POSITIVE_FIXNUM);
    let next = bignum(&mut ctx, &runtime, max + 1);

    let result = test_value(add(&mut ctx, &runtime, &[fixnum, Word::fixnum(1)]));
    assert_eq!(test_value(integer(&ctx, result)), max + 1);
    let result = test_value(sub(&mut ctx, &runtime, &[next, Word::fixnum(1)]));
    assert_eq!(test_value(integer(&ctx, result)), max);
    let result = test_value(mul(
        &mut ctx,
        &runtime,
        &[Word::fixnum(2), Word::fixnum(3), Word::fixnum(4)],
    ));
    assert_eq!(test_value(integer(&ctx, result)), 24);
    let result = test_value(sub(
        &mut ctx,
        &runtime,
        &[Word::fixnum(10), Word::fixnum(2), Word::fixnum(3)],
    ));
    assert_eq!(test_value(integer(&ctx, result)), 5);
    let result = test_value(one_plus(&mut ctx, &runtime, &[fixnum]));
    assert_eq!(test_value(integer(&ctx, result)), max + 1);
    let result = test_value(one_minus(&mut ctx, &runtime, &[next]));
    assert_eq!(test_value(integer(&ctx, result)), max);

    assert_integer_number(
        test_value(add_pair(Number::Integer(40), Number::Integer(2))),
        &Integer::from_i128(42),
    );
}

#[test]
fn arithmetic_promotes_i64_and_i128_boundary_results_without_wrapping() {
    let (runtime, mut ctx) = context();
    let min_i64 = bignum(&mut ctx, &runtime, i128::from(i64::MIN));
    assert_eq!(test_value(integer(&ctx, min_i64)), i128::from(i64::MIN));
    let min_i64_minus_one = test_value(sub(&mut ctx, &runtime, &[min_i64, Word::fixnum(1)]));
    assert_eq!(
        test_value(integer(&ctx, min_i64_minus_one)),
        i128::from(i64::MIN) - 1
    );

    let max = bignum(&mut ctx, &runtime, i128::MAX);
    let min = bignum(&mut ctx, &runtime, i128::MIN);
    assert_eq!(test_value(integer(&ctx, max)), i128::MAX);
    assert_eq!(test_value(integer(&ctx, min)), i128::MIN);

    let two_to_127 = Integer::from_limbs(false, &[0, 0, 0, 0x8000_0000]);
    let negative_two_to_127_minus_one = Integer::from_limbs(true, &[1, 0, 0, 0x8000_0000]);
    let twice_i128_max =
        Integer::from_limbs(false, &[0xffff_fffe, 0xffff_ffff, 0xffff_ffff, 0xffff_ffff]);
    assert!(two_to_127.to_i128().is_none());
    assert!(negative_two_to_127_minus_one.to_i128().is_none());
    assert!(twice_i128_max.to_i128().is_none());
    assert_integer_number(
        test_value(add_pair(Number::Integer(i128::MAX), Number::Integer(1))),
        &two_to_127,
    );
    assert_integer_number(
        test_value(sub_pair(Number::Integer(i128::MIN), Number::Integer(1))),
        &negative_two_to_127_minus_one,
    );
    assert_integer_number(
        test_value(mul_pair(Number::Integer(i128::MAX), Number::Integer(2))),
        &twice_i128_max,
    );
    assert_integer_number(
        test_value(div_pair(Number::Integer(i128::MIN), Number::Integer(-1))),
        &two_to_127,
    );

    assert_eq!(gcd_i128(i128::MIN, -1), 1);
}

#[test]
fn arithmetic_promotes_i128_overflow_and_abs_minimum() {
    let (runtime, mut ctx) = context();
    let max = bignum(&mut ctx, &runtime, i128::MAX);
    let one = Word::fixnum(1);
    let near_min = bignum(&mut ctx, &runtime, i128::MIN + 1);
    let min = bignum(&mut ctx, &runtime, i128::MIN);
    let two_to_127 = Integer::from_limbs(false, &[0, 0, 0, 0x8000_0000]);
    let negative_two_to_127_minus_one = Integer::from_limbs(true, &[1, 0, 0, 0x8000_0000]);
    let twice_i128_max =
        Integer::from_limbs(false, &[0xffff_fffe, 0xffff_ffff, 0xffff_ffff, 0xffff_ffff]);

    let result = test_value(add(&mut ctx, &runtime, &[max, one]));
    assert_eq!(test_value(arbitrary_integer(&ctx, result)), two_to_127);
    let result = test_value(sub(&mut ctx, &runtime, &[near_min, one]));
    assert_eq!(test_value(integer(&ctx, result)), i128::MIN);
    let below_min = test_value(sub(&mut ctx, &runtime, &[min, one]));
    assert_eq!(
        test_value(arbitrary_integer(&ctx, below_min)),
        negative_two_to_127_minus_one
    );
    let result = test_value(mul(&mut ctx, &runtime, &[max, Word::fixnum(2)]));
    assert_eq!(test_value(arbitrary_integer(&ctx, result)), twice_i128_max);
    assert_integer_number(
        test_value(abs_number(Number::Integer(i128::MIN))),
        &two_to_127,
    );
}

#[test]
fn comparisons_keep_pairwise_and_chain_semantics_at_boundary() {
    let (runtime, mut ctx) = context();
    let max = i128::from(crate::MOST_POSITIVE_FIXNUM);
    let fixnum = Word::fixnum(crate::MOST_POSITIVE_FIXNUM);
    let next = bignum(&mut ctx, &runtime, max + 1);
    let after = bignum(&mut ctx, &runtime, max + 2);

    assert_eq!(
        test_value(equal(&ctx, &runtime, &[fixnum, next])),
        Word::NIL
    );
    assert_eq!(
        test_value(not_equal(&ctx, &runtime, &[fixnum, next, after])),
        Word::TRUE
    );
    assert_eq!(
        test_value(less(&ctx, &runtime, &[fixnum, next, after])),
        Word::TRUE
    );
    assert_eq!(
        test_value(greater(&ctx, &runtime, &[after, next, fixnum])),
        Word::TRUE
    );
    assert_eq!(
        test_value(less_equal(&ctx, &runtime, &[fixnum, next, next])),
        Word::TRUE
    );
    assert_eq!(
        test_value(greater_equal(&ctx, &runtime, &[after, next, next])),
        Word::TRUE
    );
}

#[test]
fn registered_arithmetic_and_comparisons_use_builtin_boundary() {
    let (runtime, mut ctx) = context();
    let max = i128::from(crate::MOST_POSITIVE_FIXNUM);
    let fixnum = Word::fixnum(crate::MOST_POSITIVE_FIXNUM);
    let next = bignum(&mut ctx, &runtime, max + 1);

    let addition = call(&runtime, &mut ctx, "+", &[fixnum, Word::fixnum(1)]);
    assert_eq!(test_value(integer(&ctx, addition)), max + 1);
    let result = call(&runtime, &mut ctx, "-", &[next, Word::fixnum(1)]);
    assert_eq!(test_value(integer(&ctx, result)), max);
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
    assert_eq!(test_value(integer(&ctx, result)), max + 1);
    let result = call(&runtime, &mut ctx, "1-", &[next]);
    assert_eq!(test_value(integer(&ctx, result)), max);
    assert_eq!(call(&runtime, &mut ctx, "=", &[fixnum, next]), Word::NIL);
    assert_eq!(call(&runtime, &mut ctx, "/=", &[fixnum, next]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "<", &[fixnum, next]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, ">", &[next, fixnum]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "<=", &[fixnum, next]), Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, ">=", &[next, fixnum]), Word::TRUE);
}

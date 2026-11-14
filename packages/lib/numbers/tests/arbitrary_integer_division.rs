#![expect(
    clippy::unwrap_used,
    reason = "tests assert on numeric builtin behavior with known-valid integer fixtures"
)]
//! Division semantics with arbitrary-precision dividends and divisors.

use ncl_object::{FunctionObject, Runtime, ThreadContext, Word, make_bignum_from_limbs};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function =
        FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn integer_from_limbs(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    negative: bool,
    limbs: &[u32],
) -> Word {
    make_bignum_from_limbs(ctx, runtime, negative, limbs)
        .unwrap()
        .into()
}

fn assert_equal(runtime: &Runtime, ctx: &mut ThreadContext, left: Word, right: Word, label: &str) {
    assert_eq!(
        call(runtime, ctx, "=", &[left, right]),
        Word::TRUE,
        "{label}"
    );
}

#[derive(Clone, Copy)]
struct DivisionCase {
    label: &'static str,
    dividend: Word,
    divisor: Word,
    floor: (Word, Word),
    truncate: (Word, Word),
}

fn assert_division_case(runtime: &Runtime, ctx: &mut ThreadContext, case: DivisionCase) {
    let (floor_quotient, floor_remainder) = case.floor;
    let (truncate_quotient, truncate_remainder) = case.truncate;
    let floor = call(runtime, ctx, "FLOOR", &[case.dividend, case.divisor]);
    let actual_floor_remainder = ctx.values()[1];
    assert_equal(
        runtime,
        ctx,
        floor,
        floor_quotient,
        &format!("{}: FLOOR quotient", case.label),
    );
    assert_equal(
        runtime,
        ctx,
        actual_floor_remainder,
        floor_remainder,
        &format!("{}: FLOOR remainder", case.label),
    );
    let modulo = call(runtime, ctx, "MOD", &[case.dividend, case.divisor]);
    assert_equal(
        runtime,
        ctx,
        modulo,
        floor_remainder,
        &format!("{}: MOD remainder", case.label),
    );
    let floor_product = call(runtime, ctx, "*", &[floor, case.divisor]);
    let floor_rebuilt = call(runtime, ctx, "+", &[floor_product, modulo]);
    assert_equal(
        runtime,
        ctx,
        floor_rebuilt,
        case.dividend,
        &format!("{}: a = floor(a / b) * b + mod(a, b)", case.label),
    );

    let truncate = call(runtime, ctx, "TRUNCATE", &[case.dividend, case.divisor]);
    let actual_truncate_remainder = ctx.values()[1];
    assert_equal(
        runtime,
        ctx,
        truncate,
        truncate_quotient,
        &format!("{}: TRUNCATE quotient", case.label),
    );
    assert_equal(
        runtime,
        ctx,
        actual_truncate_remainder,
        truncate_remainder,
        &format!("{}: TRUNCATE remainder", case.label),
    );
    let rem = call(runtime, ctx, "REM", &[case.dividend, case.divisor]);
    assert_equal(
        runtime,
        ctx,
        rem,
        truncate_remainder,
        &format!("{}: REM remainder", case.label),
    );
    let truncate_product = call(runtime, ctx, "*", &[truncate, case.divisor]);
    let truncate_rebuilt = call(runtime, ctx, "+", &[truncate_product, rem]);
    assert_equal(
        runtime,
        ctx,
        truncate_rebuilt,
        case.dividend,
        &format!("{}: a = truncate(a / b) * b + rem(a, b)", case.label),
    );
}

#[test]
fn floor_truncate_rem_and_mod_match_sbcl_for_multi_limb_divisors() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10 probe inputs: a = 680564733841876926939094893764770990802
    // and b = 340282366920938463463374607421891668246.
    let a = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let negative_a = integer_from_limbs(
        &mut ctx,
        &runtime,
        true,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let b = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xb34f_e916, 0xffff_fffd, 0xffff_ffff, 0xffff_ffff],
    );
    let negative_b = integer_from_limbs(
        &mut ctx,
        &runtime,
        true,
        &[0xb34f_e916, 0xffff_fffd, 0xffff_ffff, 0xffff_ffff],
    );
    let small_remainder =
        integer_from_limbs(&mut ctx, &runtime, false, &[0x847f_38a6, 0xab54_a991]);
    let negative_small_remainder =
        integer_from_limbs(&mut ctx, &runtime, true, &[0x847f_38a6, 0xab54_a991]);
    let large_remainder = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0x2ed0_b070, 0x54ab_566c, 0xffff_ffff, 0xffff_ffff],
    );
    let negative_large_remainder = integer_from_limbs(
        &mut ctx,
        &runtime,
        true,
        &[0x2ed0_b070, 0x54ab_566c, 0xffff_ffff, 0xffff_ffff],
    );

    assert_division_case(
        &runtime,
        &mut ctx,
        DivisionCase {
            label: "a / b",
            dividend: a,
            divisor: b,
            floor: (Word::fixnum(2), small_remainder),
            truncate: (Word::fixnum(2), small_remainder),
        },
    );
    assert_division_case(
        &runtime,
        &mut ctx,
        DivisionCase {
            label: "a / -b",
            dividend: a,
            divisor: negative_b,
            floor: (Word::fixnum(-3), negative_large_remainder),
            truncate: (Word::fixnum(-2), small_remainder),
        },
    );
    assert_division_case(
        &runtime,
        &mut ctx,
        DivisionCase {
            label: "-a / b",
            dividend: negative_a,
            divisor: b,
            floor: (Word::fixnum(-3), large_remainder),
            truncate: (Word::fixnum(-2), negative_small_remainder),
        },
    );
    assert_division_case(
        &runtime,
        &mut ctx,
        DivisionCase {
            label: "-a / -b",
            dividend: negative_a,
            divisor: negative_b,
            floor: (Word::fixnum(2), negative_small_remainder),
            truncate: (Word::fixnum(2), negative_small_remainder),
        },
    );
}

#[test]
fn division_promotes_i128_minimum_overflow_to_bignum() {
    let (runtime, mut ctx) = setup();
    let minimum = integer_from_limbs(&mut ctx, &runtime, true, &[0, 0, 0, 0x8000_0000]);
    let expected = integer_from_limbs(&mut ctx, &runtime, false, &[0, 0, 0, 0x8000_0000]);
    // SBCL 2.6.10: (/ -2^127 -1) = 2^127.
    let quotient = call(&runtime, &mut ctx, "/", &[minimum, Word::fixnum(-1)]);
    assert_equal(
        &runtime,
        &mut ctx,
        quotient,
        expected,
        "i128::MIN / -1 promotes the exact quotient",
    );
}

#![expect(
    clippy::unwrap_used,
    reason = "tests assert on numeric builtin behavior with known-valid integer fixtures"
)]
//! Boundary and algebraic tests for arbitrary-precision integer arithmetic.

use ncl_object::{
    FunctionObject, Runtime, ThreadContext, Word, make_bignum_from_i128, make_bignum_from_limbs,
};

const MAX_FIXNUM: i64 = i64::MAX >> ncl_sys::FIXNUM_TAG_BITS;
const MIN_FIXNUM: i64 = i64::MIN >> ncl_sys::FIXNUM_TAG_BITS;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = function(runtime, ctx, name);
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn integer_from_i128(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Word {
    make_bignum_from_i128(ctx, runtime, value).unwrap().into()
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

fn assert_boolean(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: bool,
) {
    assert_eq!(
        call(runtime, ctx, name, args),
        if expected { Word::TRUE } else { Word::NIL },
        "{name}"
    );
}

#[test]
fn integer_operations_cross_fixnum_and_i128_boundaries() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let max_fixnum = Word::fixnum(MAX_FIXNUM);
    let min_fixnum = Word::fixnum(MIN_FIXNUM);

    let max_fixnum_plus_one = integer_from_i128(&mut ctx, &runtime, i128::from(MAX_FIXNUM) + 1);
    let min_fixnum_minus_one = integer_from_i128(&mut ctx, &runtime, i128::from(MIN_FIXNUM) - 1);
    let actual = call(&runtime, &mut ctx, "+", &[max_fixnum, one]);
    assert_equal(
        &runtime,
        &mut ctx,
        actual,
        max_fixnum_plus_one,
        "addition carries past the positive fixnum boundary",
    );
    let actual = call(&runtime, &mut ctx, "-", &[min_fixnum, one]);
    assert_equal(
        &runtime,
        &mut ctx,
        actual,
        min_fixnum_minus_one,
        "subtraction crosses the negative fixnum boundary",
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        ">",
        &[max_fixnum_plus_one, max_fixnum],
        true,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "<",
        &[min_fixnum_minus_one, min_fixnum],
        true,
    );

    let max_i128 = integer_from_i128(&mut ctx, &runtime, i128::MAX);
    let min_i128 = integer_from_i128(&mut ctx, &runtime, i128::MIN);
    let two_to_127 = integer_from_limbs(&mut ctx, &runtime, false, &[0, 0, 0, 0x8000_0000]);
    let negative_two_to_127_minus_one =
        integer_from_limbs(&mut ctx, &runtime, true, &[1, 0, 0, 0x8000_0000]);

    let actual = call(&runtime, &mut ctx, "+", &[max_i128, one]);
    assert_equal(
        &runtime,
        &mut ctx,
        actual,
        two_to_127,
        "addition carries past i128::MAX",
    );
    let actual = call(&runtime, &mut ctx, "-", &[min_i128, one]);
    assert_equal(
        &runtime,
        &mut ctx,
        actual,
        negative_two_to_127_minus_one,
        "subtraction carries past i128::MIN",
    );
    assert_boolean(&runtime, &mut ctx, ">", &[two_to_127, max_i128], true);
    assert_boolean(
        &runtime,
        &mut ctx,
        "<",
        &[negative_two_to_127_minus_one, min_i128],
        true,
    );
}

fn assert_signed_multi_limb_properties(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    a: Word,
    b: Word,
) {
    let negative_a = integer_from_limbs(ctx, runtime, true, &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2]);
    let negative_b = integer_from_limbs(
        ctx,
        runtime,
        true,
        &[0xb34f_e916, 0xffff_fffd, 0xffff_ffff, 0xffff_ffff],
    );
    let negative_product = call(runtime, ctx, "*", &[a, negative_b]);
    let expected_negative_product = integer_from_limbs(
        ctx,
        runtime,
        true,
        &[
            0xa252_100c,
            0xe11d_f6ca,
            0x7603_b105,
            0xffff_fffe,
            0x51be_dcfd,
            0xab54_a988,
            0,
            0,
            2,
        ],
    );
    assert_equal(
        runtime,
        ctx,
        negative_product,
        expected_negative_product,
        "multiplying by a negative multi-limb integer negates the SBCL product",
    );
    let reversed_negative_product = call(runtime, ctx, "*", &[negative_b, a]);
    assert_equal(
        runtime,
        ctx,
        reversed_negative_product,
        negative_product,
        "multiplication remains commutative with negative operands",
    );
    let zero = call(runtime, ctx, "+", &[a, negative_a]);
    assert_equal(runtime, ctx, zero, Word::fixnum(0), "a + (-a) = 0");
    assert_boolean(runtime, ctx, ">", &[a, b], true);
    assert_boolean(runtime, ctx, "<", &[b, a], true);
    assert_boolean(runtime, ctx, "<", &[negative_a, negative_b], true);
    assert_boolean(runtime, ctx, ">", &[negative_b, negative_a], true);
    assert_boolean(runtime, ctx, "=", &[a, a], true);
    assert_boolean(runtime, ctx, "=", &[a, negative_a], false);
}

#[test]
fn multi_limb_arithmetic_matches_sbcl_2610_and_integer_properties() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10 probe: a = 680564733841876926939094893764770990802
    // b = 340282366920938463463374607421891668246
    let a = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let b = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xb34f_e916, 0xffff_fffd, 0xffff_ffff, 0xffff_ffff],
    );
    let expected_sum = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0x9e6e_f3e8, 0xab54_a98a, 0, 0, 3],
    );
    let expected_difference = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0x37cf_21bc, 0xab54_a98f, 0, 0, 1],
    );
    let expected_product = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[
            0xa252_100c,
            0xe11d_f6ca,
            0x7603_b105,
            0xffff_fffe,
            0x51be_dcfd,
            0xab54_a988,
            0,
            0,
            2,
        ],
    );
    let actual = call(&runtime, &mut ctx, "+", &[a, b]);
    assert_equal(&runtime, &mut ctx, actual, expected_sum, "SBCL addition");
    let actual = call(&runtime, &mut ctx, "-", &[a, b]);
    assert_equal(
        &runtime,
        &mut ctx,
        actual,
        expected_difference,
        "SBCL multi-limb subtraction",
    );
    let product = call(&runtime, &mut ctx, "*", &[a, b]);
    assert_equal(
        &runtime,
        &mut ctx,
        product,
        expected_product,
        "SBCL multi-limb multiplication",
    );
    let reversed_product = call(&runtime, &mut ctx, "*", &[b, a]);
    assert_equal(
        &runtime,
        &mut ctx,
        reversed_product,
        product,
        "integer multiplication is commutative",
    );
    let recovered = call(&runtime, &mut ctx, "+", &[a, b]);
    let recovered = call(&runtime, &mut ctx, "-", &[recovered, b]);
    assert_equal(&runtime, &mut ctx, recovered, a, "(a + b) - b = a");
    assert_signed_multi_limb_properties(&runtime, &mut ctx, a, b);
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
        &format!("{}: MOD matches FLOOR remainder", case.label),
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
        &format!("{}: REM matches TRUNCATE remainder", case.label),
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
fn floor_truncate_rem_and_mod_hold_for_multi_limb_signed_dividends() {
    let (runtime, mut ctx) = setup();
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
    let q = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0x8e4c_db29, 0x43be_e4b7, 0xb8be_0547, 0x0547_41fa],
    );
    let negative_q = integer_from_limbs(
        &mut ctx,
        &runtime,
        true,
        &[0x8e4c_db29, 0x43be_e4b7, 0xb8be_0547, 0x0547_41fa],
    );
    let q_plus_one_negative = integer_from_limbs(
        &mut ctx,
        &runtime,
        true,
        &[0x8e4c_db2a, 0x43be_e4b7, 0xb8be_0547, 0x0547_41fa],
    );
    let positive_divisor = Word::fixnum(97);
    let negative_divisor = Word::fixnum(-97);

    // These quotients and remainders are from a SBCL 2.6.10 probe.
    assert_division_case(
        &runtime,
        &mut ctx,
        DivisionCase {
            label: "a / 97",
            dividend: a,
            divisor: positive_divisor,
            floor: (q, Word::fixnum(73)),
            truncate: (q, Word::fixnum(73)),
        },
    );
    assert_division_case(
        &runtime,
        &mut ctx,
        DivisionCase {
            label: "a / -97",
            dividend: a,
            divisor: negative_divisor,
            floor: (q_plus_one_negative, Word::fixnum(-24)),
            truncate: (negative_q, Word::fixnum(73)),
        },
    );
    assert_division_case(
        &runtime,
        &mut ctx,
        DivisionCase {
            label: "-a / 97",
            dividend: negative_a,
            divisor: positive_divisor,
            floor: (q_plus_one_negative, Word::fixnum(24)),
            truncate: (negative_q, Word::fixnum(-73)),
        },
    );
    assert_division_case(
        &runtime,
        &mut ctx,
        DivisionCase {
            label: "-a / -97",
            dividend: negative_a,
            divisor: negative_divisor,
            floor: (q, Word::fixnum(-73)),
            truncate: (q, Word::fixnum(-73)),
        },
    );
}

#[test]
fn ash_crosses_i128_and_uses_arithmetic_right_shift_for_negative_values() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let shifted = call(&runtime, &mut ctx, "ASH", &[one, Word::fixnum(193)]);
    let expected = integer_from_limbs(&mut ctx, &runtime, false, &[0, 0, 0, 0, 0, 0, 2]);
    assert_equal(
        &runtime,
        &mut ctx,
        shifted,
        expected,
        "ASH left shift produces a multi-limb result",
    );
    let right_shifted = call(&runtime, &mut ctx, "ASH", &[shifted, Word::fixnum(-193)]);
    assert_equal(
        &runtime,
        &mut ctx,
        right_shifted,
        one,
        "left then right shift recovers a nonnegative integer",
    );
    let shifted_negative_one = call(
        &runtime,
        &mut ctx,
        "ASH",
        &[Word::fixnum(-1), Word::fixnum(-193)],
    );
    assert_equal(
        &runtime,
        &mut ctx,
        shifted_negative_one,
        Word::fixnum(-1),
        "arithmetic right shift preserves negative sign extension",
    );
    let negative_power = integer_from_limbs(&mut ctx, &runtime, true, &[0, 0, 0, 0, 0, 0, 2]);
    let below_negative_power = integer_from_limbs(&mut ctx, &runtime, true, &[1, 0, 0, 0, 0, 0, 2]);
    let shifted_negative_power = call(
        &runtime,
        &mut ctx,
        "ASH",
        &[negative_power, Word::fixnum(-193)],
    );
    assert_equal(
        &runtime,
        &mut ctx,
        shifted_negative_power,
        Word::fixnum(-1),
        "negative power of two shifts to negative one",
    );
    let shifted_below_negative_power = call(
        &runtime,
        &mut ctx,
        "ASH",
        &[below_negative_power, Word::fixnum(-193)],
    );
    assert_equal(
        &runtime,
        &mut ctx,
        shifted_below_negative_power,
        Word::fixnum(-2),
        "negative right shift rounds toward negative infinity",
    );
}

#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert on exact numeric builtin behavior"
)]

use ncl_object::{
    Bignum, DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    bignum_limbs, bignum_sign, classify_object, complex_imag, complex_real, double_value,
    make_bignum_from_i128, make_complex, make_double, make_ratio, ratio_denominator,
    ratio_numerator,
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

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = function(runtime, ctx, name);
    runtime.call_builtin(ctx, function, args)
}

fn integer(ctx: &ThreadContext, word: Word) -> i128 {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => i128::from(value),
        ObjectRef::Bignum(value) => {
            let magnitude = bignum_limbs(ctx, Bignum::from_word(value))
                .unwrap()
                .into_iter()
                .enumerate()
                .fold(0_i128, |value, (index, limb)| {
                    value | (i128::from(limb) << (index * 32))
                });
            if bignum_sign(ctx, Bignum::from_word(value)).unwrap() {
                if magnitude == (1_i128 << 127) {
                    i128::MIN
                } else {
                    -magnitude
                }
            } else {
                magnitude
            }
        }
        other => panic!("expected integer, got {other:?}"),
    }
}

fn assert_integer(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: i128,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    assert_eq!(integer(ctx, result), expected, "{name}");
}

fn assert_boolean(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: bool,
) {
    assert_eq!(
        call(runtime, ctx, name, args).unwrap(),
        if expected { Word::TRUE } else { Word::NIL },
        "{name}"
    );
}

fn assert_float(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: f64,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    let ObjectRef::DoubleFloat(value) = classify_object(ctx, result) else {
        panic!(
            "{name}: expected double float, got {:?}",
            classify_object(ctx, result)
        );
    };
    assert_eq!(
        double_value(ctx, DoubleFloat::from_word(value)).unwrap(),
        expected,
        "{name}"
    );
}

#[test]
fn arithmetic_builtins_call_through_runtime() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    let two = Word::fixnum(2);
    let three = Word::fixnum(3);

    assert_integer(&runtime, &mut ctx, "+", &[], 0);
    assert_integer(&runtime, &mut ctx, "*", &[], 1);
    for name in ["=", "/=", "<", ">", "<=", ">="] {
        assert_boolean(&runtime, &mut ctx, name, &[], true);
        assert_boolean(&runtime, &mut ctx, name, &[two], true);
    }

    assert_integer(&runtime, &mut ctx, "+", &[one], 1);
    assert_integer(&runtime, &mut ctx, "-", &[one], -1);
    assert_integer(&runtime, &mut ctx, "*", &[three], 3);
    assert_integer(&runtime, &mut ctx, "1+", &[three], 4);
    assert_integer(&runtime, &mut ctx, "1-", &[three], 2);
    assert_boolean(&runtime, &mut ctx, "=", &[two, two, two], true);
    assert_boolean(&runtime, &mut ctx, "/=", &[one, two, three], true);

    assert_integer(&runtime, &mut ctx, "+", &[one, two, three], 6);
    assert_integer(&runtime, &mut ctx, "-", &[three, two, one], 0);
    assert_integer(&runtime, &mut ctx, "*", &[two, three, two], 12);
    assert_boolean(&runtime, &mut ctx, "<", &[one, two, three], true);
    assert_boolean(&runtime, &mut ctx, ">", &[three, two, one], true);
    assert_boolean(&runtime, &mut ctx, "<=", &[one, two, two], true);
    assert_boolean(&runtime, &mut ctx, ">=", &[three, two, two], true);

    assert_integer(
        &runtime,
        &mut ctx,
        "+",
        &[Word::fixnum(MAX_FIXNUM), one],
        i128::from(MAX_FIXNUM) + 1,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "-",
        &[Word::fixnum(MIN_FIXNUM), one],
        i128::from(MIN_FIXNUM) - 1,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "*",
        &[Word::fixnum(MAX_FIXNUM), two],
        i128::from(MAX_FIXNUM) * 2,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "1+",
        &[Word::fixnum(MAX_FIXNUM)],
        i128::from(MAX_FIXNUM) + 1,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "1-",
        &[Word::fixnum(MIN_FIXNUM)],
        i128::from(MIN_FIXNUM) - 1,
    );

    let big = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let bigger = make_bignum_from_i128(&mut ctx, &runtime, (1_i128 << 70) + 1)
        .unwrap()
        .into();
    assert_integer(&runtime, &mut ctx, "+", &[big, one], (1_i128 << 70) + 1);
    assert_integer(&runtime, &mut ctx, "-", &[big, one], (1_i128 << 70) - 1);
    assert_integer(&runtime, &mut ctx, "*", &[big, two], 2_i128 << 70);
    assert_boolean(&runtime, &mut ctx, "=", &[big, big], true);
    assert_boolean(&runtime, &mut ctx, "/=", &[big, bigger], true);
    assert_boolean(&runtime, &mut ctx, "<", &[big, bigger], true);
    assert_boolean(&runtime, &mut ctx, ">", &[bigger, big], true);
    assert_boolean(&runtime, &mut ctx, "<=", &[big, bigger], true);
    assert_boolean(&runtime, &mut ctx, ">=", &[bigger, big], true);
}

fn assert_ratio(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected_numerator: i128,
    expected_denominator: i128,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    let ObjectRef::Ratio(value) = classify_object(ctx, result) else {
        panic!(
            "{name}: expected ratio, got {:?}",
            classify_object(ctx, result)
        );
    };
    let value = ncl_object::Ratio::from_word(value);
    assert_eq!(
        integer(ctx, ratio_numerator(ctx, value).unwrap()),
        expected_numerator,
        "{name} numerator"
    );
    assert_eq!(
        integer(ctx, ratio_denominator(ctx, value).unwrap()),
        expected_denominator,
        "{name} denominator"
    );
}

/// `(n1/d1) * (n2/d2)` and `(n1/d1) / (n2/d2)` must combine numerators and
/// denominators directly; reusing the addition-shaped cross-multiplication
/// combinator here previously discarded both denominators (e.g. `2/3 * 2/3`
/// silently returned `4` instead of `4/9`).
#[test]
fn ratio_multiplication_and_division_combine_numerators_and_denominators() {
    let (runtime, mut ctx) = setup();
    let two_thirds = make_ratio(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .into();
    let five_sevenths = make_ratio(&mut ctx, &runtime, Word::fixnum(5), Word::fixnum(7))
        .unwrap()
        .into();

    assert_ratio(&runtime, &mut ctx, "*", &[two_thirds, two_thirds], 4, 9);
    assert_ratio(
        &runtime,
        &mut ctx,
        "*",
        &[two_thirds, five_sevenths],
        10,
        21,
    );
    assert_integer(&runtime, &mut ctx, "*", &[two_thirds, Word::fixnum(3)], 2);
    assert_ratio(
        &runtime,
        &mut ctx,
        "/",
        &[two_thirds, five_sevenths],
        14,
        15,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "/",
        &[Word::fixnum(1), two_thirds],
        3,
        2,
    );
}

#[test]
fn signed_ratio_arithmetic_reduces_exact_cross_products() {
    let (runtime, mut ctx) = setup();
    let left = make_ratio(&mut ctx, &runtime, Word::fixnum(-3), Word::fixnum(10))
        .unwrap()
        .into();
    let right = make_ratio(&mut ctx, &runtime, Word::fixnum(25), Word::fixnum(-9))
        .unwrap()
        .into();

    assert_ratio(&runtime, &mut ctx, "*", &[left, right], 5, 6);
    assert_ratio(&runtime, &mut ctx, "/", &[left, right], 27, 250);
}

#[test]
fn ratio_and_complex_results_survive_gc_stress_and_strict_forwarding() {
    let (runtime, mut ctx) = setup();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .into();
    let real = ncl_object::make_double(&mut ctx, &runtime, 2.0)
        .unwrap()
        .into();
    let imag = ncl_object::make_double(&mut ctx, &runtime, 3.0)
        .unwrap()
        .into();
    let complex = ncl_object::make_complex(&mut ctx, &runtime, real, imag)
        .unwrap()
        .into();
    let function = function(&runtime, &mut ctx, "+");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let mut ratio = ratio;
    let token = ncl_object::push_root(&mut ctx, &mut ratio);
    let mut complex = complex;
    let complex_token = ncl_object::push_root(&mut ctx, &mut complex);
    let result = runtime
        .call_builtin(&mut ctx, function, &[ratio, Word::fixnum(1)])
        .unwrap();
    let ObjectRef::Ratio(value) = classify_object(&ctx, result) else {
        panic!(
            "expected ratio result, got {:?}",
            classify_object(&ctx, result)
        );
    };
    let ratio = ncl_object::Ratio::from_word(value);
    assert_eq!(integer(&ctx, ratio_numerator(&ctx, ratio).unwrap()), 3);
    assert_eq!(integer(&ctx, ratio_denominator(&ctx, ratio).unwrap()), 2);
    let complex_result = runtime
        .call_builtin(&mut ctx, function, &[complex, Word::fixnum(1)])
        .unwrap();
    let ObjectRef::Complex(complex_result) = classify_object(&ctx, complex_result) else {
        panic!("expected complex result after addition");
    };
    let complex_result = ncl_object::Complex::from_word(complex_result);
    let real = complex_real(&ctx, complex_result).unwrap();
    let imag = complex_imag(&ctx, complex_result).unwrap();
    let ObjectRef::DoubleFloat(real) = classify_object(&ctx, real) else {
        panic!("expected double-float real component");
    };
    let ObjectRef::DoubleFloat(imag) = classify_object(&ctx, imag) else {
        panic!("expected double-float imaginary component");
    };
    assert_eq!(
        double_value(&ctx, DoubleFloat::from_word(real)).unwrap(),
        3.0
    );
    assert_eq!(
        double_value(&ctx, DoubleFloat::from_word(imag)).unwrap(),
        3.0
    );
    assert!(ncl_object::pop_root(&mut ctx, complex_token));
    assert!(ncl_object::pop_root(&mut ctx, token));
}

#[test]
fn unary_arithmetic_handles_zero_division_absolute_and_sign_values() {
    let (runtime, mut ctx) = setup();
    assert_ratio(&runtime, &mut ctx, "/", &[Word::fixnum(2)], 1, 2);
    assert_eq!(
        call(&runtime, &mut ctx, "/", &[Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );

    let negative_ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(-3), Word::fixnum(2))
        .unwrap()
        .into();
    assert_integer(&runtime, &mut ctx, "SIGNUM", &[negative_ratio], -1);
    assert_ratio(&runtime, &mut ctx, "ABS", &[negative_ratio], 3, 2);

    let real = make_double(&mut ctx, &runtime, 3.0).unwrap().into();
    let imag = make_double(&mut ctx, &runtime, 4.0).unwrap().into();
    let complex = make_complex(&mut ctx, &runtime, real, imag).unwrap().into();
    let magnitude = call(&runtime, &mut ctx, "ABS", &[complex]).unwrap();
    let ObjectRef::DoubleFloat(value) = classify_object(&ctx, magnitude) else {
        panic!("expected complex ABS to return a float");
    };
    assert_eq!(
        ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(value)).unwrap(),
        5.0
    );
}

#[test]
fn arithmetic_reports_i128_boundary_overflow_without_wrapping() {
    let (runtime, mut ctx) = setup();
    let min = make_bignum_from_i128(&mut ctx, &runtime, i128::MIN)
        .unwrap()
        .into();
    let max = make_bignum_from_i128(&mut ctx, &runtime, i128::MAX)
        .unwrap()
        .into();
    let one = Word::fixnum(1);
    let minus_one = Word::fixnum(-1);

    for (name, args) in [
        ("+", vec![max, one]),
        ("-", vec![min, one]),
        ("*", vec![max, Word::fixnum(2)]),
        ("/", vec![min, minus_one]),
        ("ABS", vec![min]),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &args),
            Err(ObjectError::TypeError),
            "{name} must reject an unrepresentable i128 result",
        );
    }
}

#[test]
fn signum_and_abs_preserve_zero_contracts() {
    let (runtime, mut ctx) = setup();
    assert_integer(&runtime, &mut ctx, "SIGNUM", &[Word::fixnum(0)], 0);
    assert_integer(&runtime, &mut ctx, "ABS", &[Word::fixnum(0)], 0);

    let negative = make_double(&mut ctx, &runtime, -2.5).unwrap().into();
    let signum = call(&runtime, &mut ctx, "SIGNUM", &[negative]).unwrap();
    let ObjectRef::DoubleFloat(signum) = classify_object(&ctx, signum) else {
        panic!("SIGNUM of a float must remain a float");
    };
    assert_eq!(
        ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(signum))
            .unwrap()
            .to_bits(),
        (-1.0_f64).to_bits()
    );
}

#[test]
fn mixed_numeric_comparisons_and_extrema_return_exact_values() {
    let (runtime, mut ctx) = setup();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let same_ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let float = make_double(&mut ctx, &runtime, 1.5).unwrap().into();
    let same_float = make_double(&mut ctx, &runtime, 1.5).unwrap().into();
    let real = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    let imag = make_double(&mut ctx, &runtime, -3.0).unwrap().into();
    let complex = make_complex(&mut ctx, &runtime, real, imag).unwrap().into();
    let same_complex = make_complex(&mut ctx, &runtime, real, imag).unwrap().into();
    let bignum = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let same_bignum = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();

    assert_boolean(&runtime, &mut ctx, "=", &[ratio, float], true);
    assert_boolean(&runtime, &mut ctx, "=", &[float, Word::fixnum(2)], false);
    assert_boolean(&runtime, &mut ctx, "/=", &[ratio, float], false);
    assert_boolean(
        &runtime,
        &mut ctx,
        "EQ",
        &[Word::fixnum(7), Word::fixnum(7)],
        true,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "EQ",
        &[Word::fixnum(7), Word::fixnum(8)],
        false,
    );
    assert_boolean(&runtime, &mut ctx, "EQL", &[same_bignum, bignum], true);
    assert_boolean(&runtime, &mut ctx, "EQL", &[same_ratio, ratio], true);
    assert_boolean(&runtime, &mut ctx, "EQL", &[same_float, float], true);
    assert_boolean(&runtime, &mut ctx, "EQL", &[same_complex, complex], true);
    assert_boolean(&runtime, &mut ctx, "EQL", &[ratio, float], false);

    assert_integer(&runtime, &mut ctx, "MAX", &[ratio, Word::fixnum(2)], 2);
    assert_ratio(&runtime, &mut ctx, "MIN", &[ratio, Word::fixnum(2)], 3, 2);
    assert_float(&runtime, &mut ctx, "MAX", &[float, ratio], 1.5);
    assert_float(&runtime, &mut ctx, "MIN", &[float, Word::fixnum(2)], 1.5);

    assert_boolean(&runtime, &mut ctx, "ZEROP", &[complex], false);
    assert_boolean(&runtime, &mut ctx, "PLUSP", &[float], true);
    assert_boolean(&runtime, &mut ctx, "MINUSP", &[float], false);
    assert_boolean(&runtime, &mut ctx, "EVENP", &[bignum], true);
    assert_boolean(&runtime, &mut ctx, "ODDP", &[bignum], false);
}

#[test]
fn bignum_and_ratio_arithmetic_preserves_exact_boundary_values() {
    let (runtime, mut ctx) = setup();
    let minimum = make_bignum_from_i128(&mut ctx, &runtime, i128::MIN)
        .unwrap()
        .into();
    let maximum = make_bignum_from_i128(&mut ctx, &runtime, i128::MAX)
        .unwrap()
        .into();
    assert_integer(
        &runtime,
        &mut ctx,
        "+",
        &[minimum, Word::fixnum(1)],
        -170_141_183_460_469_231_731_687_303_715_884_105_727,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "-",
        &[maximum, Word::fixnum(1)],
        170_141_183_460_469_231_731_687_303_715_884_105_726,
    );

    let bignum = make_bignum_from_i128(&mut ctx, &runtime, 1_180_591_620_717_411_303_425)
        .unwrap()
        .into();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    assert_ratio(
        &runtime,
        &mut ctx,
        "+",
        &[bignum, ratio],
        2_361_183_241_434_822_606_853,
        2,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "*",
        &[bignum, ratio],
        3_541_774_862_152_233_910_275,
        2,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "/",
        &[bignum, ratio],
        2_361_183_241_434_822_606_850,
        3,
    );
}

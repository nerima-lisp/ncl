#![expect(
    clippy::unwrap_used,
    reason = "tests assert on rounding and integer-conversion boundaries"
)]
//! Tests for numeric rounding rules and integer-conversion boundaries.

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, double_value, make_bignum_from_limbs,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}
fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function =
        FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap();
    runtime.call_builtin(ctx, function, args)
}
fn integer(ctx: &ThreadContext, word: Word) -> i128 {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => i128::from(value),
        ObjectRef::Bignum(value) => {
            let limbs =
                ncl_object::bignum_limbs(ctx, ncl_object::Bignum::from_word(value)).unwrap();
            let magnitude = limbs
                .into_iter()
                .enumerate()
                .fold(0_i128, |value, (index, limb)| {
                    value | (i128::from(limb) << (index * 32))
                });
            if ncl_object::bignum_sign(ctx, ncl_object::Bignum::from_word(value)).unwrap() {
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
fn float(ctx: &ThreadContext, word: Word) -> f64 {
    match classify_object(ctx, word) {
        ObjectRef::DoubleFloat(value) => double_value(ctx, DoubleFloat::from_word(value)).unwrap(),
        other => panic!("expected float, got {other:?}"),
    }
}

fn ratio(runtime: &Runtime, ctx: &mut ThreadContext, numerator: i64, denominator: i64) -> Word {
    call(
        runtime,
        ctx,
        "/",
        &[Word::fixnum(numerator), Word::fixnum(denominator)],
    )
    .unwrap()
}

fn integer_from_limbs(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    negative: bool,
    limbs: &[u32],
) -> Word {
    make_bignum_from_limbs(ctx, runtime, negative, limbs)
        .unwrap()
        .into()
}

fn assert_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    actual: Word,
    expected: Word,
    label: &str,
) {
    assert_eq!(
        call(runtime, ctx, "=", &[actual, expected]).unwrap(),
        Word::TRUE,
        "{label}"
    );
}

struct RoundCase {
    numerator: Word,
    divisor: Word,
    quotient: i128,
    remainder: Word,
    integer_remainder: bool,
    label: &'static str,
}

fn assert_round_case(runtime: &Runtime, ctx: &mut ThreadContext, case: RoundCase) {
    let quotient = call(runtime, ctx, "ROUND", &[case.numerator, case.divisor]).unwrap();
    let mut remainder = ctx.values()[1];
    let remainder_root = ncl_object::push_root(ctx, &mut remainder);
    assert_eq!(
        integer(ctx, quotient),
        case.quotient,
        "{} quotient",
        case.label
    );
    assert_eq!(
        call(runtime, ctx, "INTEGERP", &[quotient]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        call(runtime, ctx, "RATIONALP", &[remainder]).unwrap(),
        Word::TRUE,
        "{} remainder type",
        case.label,
    );
    assert_eq!(
        call(runtime, ctx, "INTEGERP", &[remainder]).unwrap(),
        if case.integer_remainder {
            Word::TRUE
        } else {
            Word::NIL
        },
        "{} remainder integer type",
        case.label,
    );
    assert_equal(runtime, ctx, remainder, case.remainder, case.label);

    let mut product = call(runtime, ctx, "*", &[quotient, case.divisor]).unwrap();
    let product_root = ncl_object::push_root(ctx, &mut product);
    let mut reconstructed = call(runtime, ctx, "+", &[product, remainder]).unwrap();
    let reconstructed_root = ncl_object::push_root(ctx, &mut reconstructed);
    assert_equal(runtime, ctx, reconstructed, case.numerator, case.label);

    assert!(ncl_object::pop_root(ctx, reconstructed_root));
    assert!(ncl_object::pop_root(ctx, product_root));
    assert!(ncl_object::pop_root(ctx, remainder_root));
}

#[test]
fn rounding_returns_signed_quotient_and_remainder_values() {
    let (runtime, mut ctx) = setup();
    for (name, quotient, remainder) in [("FLOOR", -4, 1), ("CEILING", -3, -1), ("TRUNCATE", -3, -1)]
    {
        let result = call(
            &runtime,
            &mut ctx,
            name,
            &[Word::fixnum(-7), Word::fixnum(2)],
        )
        .unwrap();
        assert_eq!(integer(&ctx, result), quotient, "{name} quotient");
        assert_eq!(
            integer(&ctx, ctx.values()[1]),
            remainder,
            "{name} remainder"
        );
    }
    let result = call(
        &runtime,
        &mut ctx,
        "ROUND",
        &[Word::fixnum(7), Word::fixnum(2)],
    )
    .unwrap();
    assert_eq!(integer(&ctx, result), 4);
    assert_eq!(integer(&ctx, ctx.values()[1]), -1);
}

#[test]
fn float_rounding_keeps_float_types_and_tolerates_binary_error() {
    let (runtime, mut ctx) = setup();
    let value = ncl_object::make_double(&mut ctx, &runtime, -7.5)
        .unwrap()
        .into();
    let divisor = ncl_object::make_double(&mut ctx, &runtime, 2.0)
        .unwrap()
        .into();
    let quotient = call(&runtime, &mut ctx, "FFLOOR", &[value, divisor]).unwrap();
    assert!(matches!(
        classify_object(&ctx, quotient),
        ObjectRef::DoubleFloat(_)
    ));
    assert!(matches!(
        classify_object(&ctx, ctx.values()[1]),
        ObjectRef::DoubleFloat(_)
    ));
    assert!((float(&ctx, quotient) - -4.0).abs() < 1e-12);
    assert!((float(&ctx, ctx.values()[1]) - 0.5).abs() < 1e-12);
}

#[test]
fn unary_float_rounding_returns_expected_quotient_and_remainder() {
    let (runtime, mut ctx) = setup();
    let value = ncl_object::make_double(&mut ctx, &runtime, -2.5)
        .unwrap()
        .into();

    for (name, quotient, remainder, float_quotient) in [
        ("FLOOR", -3, 0.5, false),
        ("CEILING", -2, -0.5, false),
        ("TRUNCATE", -2, -0.5, false),
        ("ROUND", -2, -0.5, false),
        ("FFLOOR", -3, 0.5, true),
        ("FCEILING", -2, -0.5, true),
        ("FTRUNCATE", -2, -0.5, true),
        ("FROUND", -2, -0.5, true),
    ] {
        let result = call(&runtime, &mut ctx, name, &[value]).unwrap();
        if float_quotient {
            assert!(matches!(
                classify_object(&ctx, result),
                ObjectRef::DoubleFloat(_)
            ));
            assert!(
                (float(&ctx, result) - f64::from(quotient)).abs() < 1e-12,
                "{name} quotient"
            );
        } else {
            assert_eq!(
                integer(&ctx, result),
                i128::from(quotient),
                "{name} quotient"
            );
        }
        assert!(matches!(
            classify_object(&ctx, ctx.values()[1]),
            ObjectRef::DoubleFloat(_)
        ));
        assert!(
            (float(&ctx, ctx.values()[1]) - remainder).abs() < 1e-12,
            "{name} remainder"
        );
    }

    let tie = ncl_object::make_double(&mut ctx, &runtime, -1.5)
        .unwrap()
        .into();
    for (name, float_quotient) in [("ROUND", false), ("FROUND", true)] {
        let result = call(&runtime, &mut ctx, name, &[tie]).unwrap();
        if float_quotient {
            assert!(matches!(
                classify_object(&ctx, result),
                ObjectRef::DoubleFloat(_)
            ));
            assert!(
                (float(&ctx, result) + 2.0).abs() < 1e-12,
                "{name} tie quotient"
            );
        } else {
            assert_eq!(integer(&ctx, result), -2, "{name} tie quotient");
        }
        assert!(matches!(
            classify_object(&ctx, ctx.values()[1]),
            ObjectRef::DoubleFloat(_)
        ));
        assert!(
            (float(&ctx, ctx.values()[1]) - 0.5).abs() < 1e-12,
            "{name} tie remainder"
        );
    }
}

#[test]
fn float_rounding_returns_bignum_quotients_above_i128() {
    let (runtime, mut ctx) = setup();
    let value = ncl_object::make_double(&mut ctx, &runtime, 2.0_f64.powi(128))
        .unwrap()
        .into();
    let quotient = call(&runtime, &mut ctx, "FLOOR", &[value]).unwrap();
    let ObjectRef::Bignum(bignum) = classify_object(&ctx, quotient) else {
        panic!("expected a bignum quotient");
    };
    assert_eq!(
        ncl_object::bignum_limbs(&ctx, ncl_object::Bignum::from_word(bignum)).unwrap(),
        [0, 0, 0, 0, 1]
    );
    assert!(!ncl_object::bignum_sign(&ctx, ncl_object::Bignum::from_word(bignum)).unwrap());
    assert!(float(&ctx, ctx.values()[1]).abs() < 1e-12);
}

#[test]
fn rounding_rejects_zero_divisor() {
    let (runtime, mut ctx) = setup();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "FLOOR",
            &[Word::fixnum(1), Word::fixnum(0)]
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn rounding_preserves_i128_min_without_negation_overflow() {
    let (runtime, mut ctx) = setup();
    let min = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, i128::MIN)
        .unwrap()
        .into();
    let one = Word::fixnum(1);

    for name in ["FLOOR", "CEILING", "TRUNCATE", "ROUND"] {
        let result = call(&runtime, &mut ctx, name, &[min, one]).unwrap();
        assert_eq!(integer(&ctx, result), i128::MIN, "{name} quotient");
        assert_eq!(integer(&ctx, ctx.values()[1]), 0, "{name} remainder");
    }
}

#[test]
fn rounding_promotes_i128_min_negation_to_bignum() {
    let (runtime, mut ctx) = setup();
    let min = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, i128::MIN)
        .unwrap()
        .into();
    let minus_one = Word::fixnum(-1);

    for name in ["FLOOR", "CEILING", "TRUNCATE", "ROUND"] {
        let result = call(&runtime, &mut ctx, name, &[min, minus_one]).unwrap();
        let bignum = match classify_object(&ctx, result) {
            ObjectRef::Bignum(value) => ncl_object::Bignum::from_word(value),
            other => panic!("expected bignum quotient, got {other:?}"),
        };
        assert_eq!(
            ncl_object::bignum_limbs(&ctx, bignum).unwrap(),
            [0, 0, 0, 0x8000_0000],
            "{name} quotient magnitude",
        );
        assert!(
            !ncl_object::bignum_sign(&ctx, bignum).unwrap(),
            "{name} quotient sign"
        );
        assert_eq!(integer(&ctx, ctx.values()[1]), 0, "{name} remainder");
    }
}

#[test]
fn multi_limb_round_ties_to_even_for_opposite_signs() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: b=2^128, tie=3*b+b/2; ROUND returns +/-4 and signed b/2.
    let mut b = integer_from_limbs(&runtime, &mut ctx, false, &[0, 0, 0, 0, 1]);
    let b_root = ncl_object::push_root(&mut ctx, &mut b);
    let mut negative_b = integer_from_limbs(&runtime, &mut ctx, true, &[0, 0, 0, 0, 1]);
    let negative_b_root = ncl_object::push_root(&mut ctx, &mut negative_b);
    let tie_limbs = [0, 0, 0, 0x8000_0000, 3];
    let mut tie = integer_from_limbs(&runtime, &mut ctx, false, &tie_limbs);
    let tie_root = ncl_object::push_root(&mut ctx, &mut tie);
    let mut negative_tie = integer_from_limbs(&runtime, &mut ctx, true, &tie_limbs);
    let negative_tie_root = ncl_object::push_root(&mut ctx, &mut negative_tie);
    let half_limbs = [0, 0, 0, 0x8000_0000];
    let mut half = integer_from_limbs(&runtime, &mut ctx, false, &half_limbs);
    let half_root = ncl_object::push_root(&mut ctx, &mut half);
    let mut negative_half = integer_from_limbs(&runtime, &mut ctx, true, &half_limbs);
    let negative_half_root = ncl_object::push_root(&mut ctx, &mut negative_half);

    for case in [
        RoundCase {
            numerator: tie,
            divisor: negative_b,
            quotient: -4,
            remainder: negative_half,
            integer_remainder: true,
            label: "ROUND(3.5b,-b)",
        },
        RoundCase {
            numerator: negative_tie,
            divisor: b,
            quotient: -4,
            remainder: half,
            integer_remainder: true,
            label: "ROUND(-3.5b,b)",
        },
        RoundCase {
            numerator: tie,
            divisor: b,
            quotient: 4,
            remainder: negative_half,
            integer_remainder: true,
            label: "ROUND(3.5b,b)",
        },
        RoundCase {
            numerator: negative_tie,
            divisor: negative_b,
            quotient: 4,
            remainder: half,
            integer_remainder: true,
            label: "ROUND(-3.5b,-b)",
        },
    ] {
        assert_round_case(&runtime, &mut ctx, case);
    }

    assert!(ncl_object::pop_root(&mut ctx, negative_half_root));
    assert!(ncl_object::pop_root(&mut ctx, half_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_tie_root));
    assert!(ncl_object::pop_root(&mut ctx, tie_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_b_root));
    assert!(ncl_object::pop_root(&mut ctx, b_root));
}

#[test]
fn ratio_round_ties_to_even_with_integer_divisor() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: ROUND(7/2,1)=(4,-1/2), ROUND(5/2,1)=(2,1/2),
    // with opposite signs producing (-4,1/2) and (-2,-1/2).
    let mut seven_halves = ratio(&runtime, &mut ctx, 7, 2);
    let seven_halves_root = ncl_object::push_root(&mut ctx, &mut seven_halves);
    let mut negative_seven_halves = ratio(&runtime, &mut ctx, -7, 2);
    let negative_seven_halves_root = ncl_object::push_root(&mut ctx, &mut negative_seven_halves);
    let mut five_halves = ratio(&runtime, &mut ctx, 5, 2);
    let five_halves_root = ncl_object::push_root(&mut ctx, &mut five_halves);
    let mut negative_five_halves = ratio(&runtime, &mut ctx, -5, 2);
    let negative_five_halves_root = ncl_object::push_root(&mut ctx, &mut negative_five_halves);
    let mut half = ratio(&runtime, &mut ctx, 1, 2);
    let half_root = ncl_object::push_root(&mut ctx, &mut half);
    let mut negative_half = ratio(&runtime, &mut ctx, -1, 2);
    let negative_half_root = ncl_object::push_root(&mut ctx, &mut negative_half);
    let one = Word::fixnum(1);

    for case in [
        RoundCase {
            numerator: seven_halves,
            divisor: one,
            quotient: 4,
            remainder: negative_half,
            integer_remainder: false,
            label: "ROUND(7/2,1)",
        },
        RoundCase {
            numerator: negative_seven_halves,
            divisor: one,
            quotient: -4,
            remainder: half,
            integer_remainder: false,
            label: "ROUND(-7/2,1)",
        },
        RoundCase {
            numerator: five_halves,
            divisor: one,
            quotient: 2,
            remainder: half,
            integer_remainder: false,
            label: "ROUND(5/2,1)",
        },
        RoundCase {
            numerator: negative_five_halves,
            divisor: one,
            quotient: -2,
            remainder: negative_half,
            integer_remainder: false,
            label: "ROUND(-5/2,1)",
        },
    ] {
        assert_round_case(&runtime, &mut ctx, case);
    }

    assert!(ncl_object::pop_root(&mut ctx, negative_half_root));
    assert!(ncl_object::pop_root(&mut ctx, half_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_five_halves_root));
    assert!(ncl_object::pop_root(&mut ctx, five_halves_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_seven_halves_root));
    assert!(ncl_object::pop_root(&mut ctx, seven_halves_root));
}

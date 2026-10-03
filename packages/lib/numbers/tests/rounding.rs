#![allow(clippy::float_cmp, clippy::unwrap_used, missing_docs)]

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, double_value, ratio_denominator, ratio_numerator,
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
    assert_eq!(float(&ctx, quotient), -4.0);
    assert_eq!(float(&ctx, ctx.values()[1]), 0.5);
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
fn rounding_reports_unrepresentable_i128_min_negation() {
    let (runtime, mut ctx) = setup();
    let min = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, i128::MIN)
        .unwrap()
        .into();
    let minus_one = Word::fixnum(-1);

    for name in ["FLOOR", "CEILING", "TRUNCATE", "ROUND"] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[min, minus_one]),
            Err(ObjectError::TypeError),
            "{name} result",
        );
    }
}

#[test]
fn rounding_covers_ratio_divisors_and_float_result_variants() {
    let (runtime, mut ctx) = setup();
    let value = ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(7), Word::fixnum(2))
        .unwrap()
        .into();
    let divisor = ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let quotient = call(&runtime, &mut ctx, "FLOOR", &[value, divisor]).unwrap();
    assert_eq!(integer(&ctx, quotient), 2);
    let remainder = ctx.values()[1];
    let ObjectRef::Ratio(remainder) = classify_object(&ctx, remainder) else {
        panic!(
            "expected ratio remainder, got {:?}",
            classify_object(&ctx, remainder)
        );
    };
    let remainder = ncl_object::Ratio::from_word(remainder);
    assert_eq!(integer(&ctx, ratio_numerator(&ctx, remainder).unwrap()), 1);
    assert_eq!(
        integer(&ctx, ratio_denominator(&ctx, remainder).unwrap()),
        2
    );

    let value = ncl_object::make_double(&mut ctx, &runtime, 2.5)
        .unwrap()
        .into();
    let divisor = ncl_object::make_double(&mut ctx, &runtime, 1.0)
        .unwrap()
        .into();
    for (name, expected_quotient, expected_remainder) in [
        ("FCEILING", 3.0, -0.5),
        ("FTRUNCATE", 2.0, 0.5),
        ("FROUND", 2.0, 0.5),
    ] {
        let quotient = call(&runtime, &mut ctx, name, &[value, divisor]).unwrap();
        assert_eq!(float(&ctx, quotient), expected_quotient, "{name} quotient");
        assert_eq!(
            float(&ctx, ctx.values()[1]),
            expected_remainder,
            "{name} remainder"
        );
    }
}

#[test]
fn rounding_observes_negative_divisor_and_half_even_contracts() {
    let (runtime, mut ctx) = setup();
    let divisor = Word::fixnum(-2);
    for (name, quotient, remainder) in [
        ("FLOOR", -4, -1),
        ("CEILING", -3, 1),
        ("TRUNCATE", -3, 1),
        ("ROUND", -4, -1),
    ] {
        let result = call(&runtime, &mut ctx, name, &[Word::fixnum(7), divisor]).unwrap();
        assert_eq!(integer(&ctx, result), quotient, "{name} quotient");
        assert_eq!(
            integer(&ctx, ctx.values()[1]),
            remainder,
            "{name} remainder"
        );
    }

    for (value, quotient, remainder) in [(5, 2, 1), (7, 4, -1), (-5, -2, -1), (-7, -4, 1)] {
        let result = call(
            &runtime,
            &mut ctx,
            "ROUND",
            &[Word::fixnum(value), Word::fixnum(2)],
        )
        .unwrap();
        assert_eq!(
            integer(&ctx, result),
            quotient,
            "ROUND({value}, 2) quotient"
        );
        assert_eq!(
            integer(&ctx, ctx.values()[1]),
            remainder,
            "ROUND({value}, 2) remainder"
        );
    }
}

#[test]
fn rounding_uses_one_as_the_default_divisor() {
    let (runtime, mut ctx) = setup();
    for name in ["FLOOR", "CEILING", "TRUNCATE", "ROUND"] {
        let result = call(&runtime, &mut ctx, name, &[Word::fixnum(-7)]).unwrap();
        assert_eq!(integer(&ctx, result), -7, "{name} quotient");
        assert_eq!(integer(&ctx, ctx.values()[1]), 0, "{name} remainder");
    }
}

#[test]
fn float_round_uses_half_even_and_rejects_zero_divisors() {
    let (runtime, mut ctx) = setup();
    let one = ncl_object::make_double(&mut ctx, &runtime, 1.0)
        .unwrap()
        .into();
    for (value, quotient, remainder) in [(2.5_f64, 2.0_f64, 0.5_f64), (3.5_f64, 4.0_f64, -0.5_f64)]
    {
        let value = ncl_object::make_double(&mut ctx, &runtime, value)
            .unwrap()
            .into();
        let result = call(&runtime, &mut ctx, "FROUND", &[value, one]).unwrap();
        assert_eq!(float(&ctx, result).to_bits(), quotient.to_bits());
        assert_eq!(float(&ctx, ctx.values()[1]).to_bits(), remainder.to_bits());
    }

    let value = ncl_object::make_double(&mut ctx, &runtime, 1.0)
        .unwrap()
        .into();
    let zero = ncl_object::make_double(&mut ctx, &runtime, 0.0)
        .unwrap()
        .into();
    assert_eq!(
        call(&runtime, &mut ctx, "FFLOOR", &[value, zero]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn rounding_ties_negative_divisors_and_bignum_ratios_have_exact_results() {
    let (runtime, mut ctx) = setup();
    for (value, divisor, quotient, remainder) in
        [(5, 2, 2, 1), (3, 2, 2, -1), (-5, 2, -2, -1), (-3, 2, -2, 1)]
    {
        let result = call(
            &runtime,
            &mut ctx,
            "ROUND",
            &[Word::fixnum(value), Word::fixnum(divisor)],
        )
        .unwrap();
        assert_eq!(integer(&ctx, result), quotient, "ROUND {value}/{divisor}");
        assert_eq!(integer(&ctx, ctx.values()[1]), remainder, "ROUND remainder");
    }

    for (name, quotient, remainder) in [("FLOOR", -4, -1), ("CEILING", -3, 1), ("TRUNCATE", -3, 1)]
    {
        let result = call(
            &runtime,
            &mut ctx,
            name,
            &[Word::fixnum(7), Word::fixnum(-2)],
        )
        .unwrap();
        assert_eq!(integer(&ctx, result), quotient, "{name} quotient");
        assert_eq!(
            integer(&ctx, ctx.values()[1]),
            remainder,
            "{name} remainder"
        );
    }

    let bignum = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, (1_i128 << 70) + 1)
        .unwrap()
        .into();
    let ratio = ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let result = call(&runtime, &mut ctx, "FLOOR", &[bignum, ratio]).unwrap();
    assert_eq!(integer(&ctx, result), 787_061_080_478_274_202_283);
    let remainder = ctx.values()[1];
    let ObjectRef::Ratio(remainder) = classify_object(&ctx, remainder) else {
        panic!(
            "expected ratio remainder, got {:?}",
            classify_object(&ctx, remainder)
        );
    };
    let remainder = ncl_object::Ratio::from_word(remainder);
    assert_eq!(integer(&ctx, ratio_numerator(&ctx, remainder).unwrap()), 1);
    assert_eq!(
        integer(&ctx, ratio_denominator(&ctx, remainder).unwrap()),
        2
    );

    let value = ncl_object::make_double(&mut ctx, &runtime, 3.5)
        .unwrap()
        .into();
    let divisor = ncl_object::make_double(&mut ctx, &runtime, 1.0)
        .unwrap()
        .into();
    let result = call(&runtime, &mut ctx, "FROUND", &[value, divisor]).unwrap();
    assert_eq!(float(&ctx, result), 4.0);
    assert_eq!(float(&ctx, ctx.values()[1]), -0.5);
}

#[test]
fn rounding_ratio_divisors_preserve_signed_exact_remainders() {
    let (runtime, mut ctx) = setup();
    let value = ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(-7), Word::fixnum(3))
        .unwrap()
        .into();
    let divisor = ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(-2), Word::fixnum(5))
        .unwrap()
        .into();

    for (name, quotient, numerator, denominator) in [
        ("FLOOR", 5, -1, 3),
        ("CEILING", 6, 1, 15),
        ("TRUNCATE", 5, -1, 3),
        ("ROUND", 6, 1, 15),
    ] {
        let result = call(&runtime, &mut ctx, name, &[value, divisor]).unwrap();
        assert_eq!(integer(&ctx, result), quotient, "{name} quotient");
        let remainder = ctx.values()[1];
        let ObjectRef::Ratio(remainder) = classify_object(&ctx, remainder) else {
            panic!("{name} remainder must remain an exact ratio");
        };
        let remainder = ncl_object::Ratio::from_word(remainder);
        assert_eq!(
            integer(&ctx, ratio_numerator(&ctx, remainder).unwrap()),
            numerator
        );
        assert_eq!(
            integer(&ctx, ratio_denominator(&ctx, remainder).unwrap()),
            denominator
        );
    }
}

#[test]
fn float_rounding_returns_ansi_quotients_and_remainders() {
    let (runtime, mut ctx) = setup();
    let value = ncl_object::make_double(&mut ctx, &runtime, 7.0)
        .unwrap()
        .into();
    let divisor = ncl_object::make_double(&mut ctx, &runtime, 2.0)
        .unwrap()
        .into();

    for (name, quotient, remainder) in [
        ("FLOOR", 3.0, 1.0),
        ("FCEILING", 4.0, -1.0),
        ("FTRUNCATE", 3.0, 1.0),
        ("FROUND", 4.0, -1.0),
    ] {
        let result = call(&runtime, &mut ctx, name, &[value, divisor]).unwrap();
        assert_eq!(float(&ctx, result), quotient, "{name} quotient");
        assert_eq!(float(&ctx, ctx.values()[1]), remainder, "{name} remainder");
    }
}

#[test]
fn rounding_rejects_non_numbers_and_exact_cross_product_overflow() {
    let (runtime, mut ctx) = setup();
    for name in [
        "FLOOR",
        "CEILING",
        "TRUNCATE",
        "ROUND",
        "FFLOOR",
        "FCEILING",
        "FTRUNCATE",
        "FROUND",
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[Word::TRUE]),
            Err(ObjectError::TypeError),
            "{name} must reject a non-number"
        );
    }

    let numerator = ncl_object::make_bignum_from_i128(&mut ctx, &runtime, i128::MAX)
        .unwrap()
        .into();
    let value = ncl_object::make_ratio(&mut ctx, &runtime, numerator, Word::fixnum(2))
        .unwrap()
        .into();
    let divisor = ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2))
        .unwrap()
        .into();
    for name in ["FLOOR", "CEILING", "TRUNCATE", "ROUND"] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[value, divisor]),
            Err(ObjectError::TypeError),
            "{name} reports exact cross-product overflow"
        );
    }
}

#[test]
fn exact_rounding_preserves_signs_for_negative_ratio_divisors() {
    let (runtime, mut ctx) = setup();
    let value = ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(7), Word::fixnum(1))
        .unwrap()
        .into();
    let divisor = ncl_object::make_ratio(&mut ctx, &runtime, Word::fixnum(-3), Word::fixnum(2))
        .unwrap()
        .into();

    for (name, quotient, remainder_numerator, remainder_denominator) in [
        ("FLOOR", -5, -1, 2),
        ("CEILING", -4, 1, 1),
        ("TRUNCATE", -4, 1, 1),
        ("ROUND", -5, -1, 2),
    ] {
        let result = call(&runtime, &mut ctx, name, &[value, divisor]).unwrap();
        assert_eq!(integer(&ctx, result), quotient, "{name} quotient");
        let remainder = ctx.values()[1];
        if remainder_denominator == 1 {
            assert_eq!(
                integer(&ctx, remainder),
                remainder_numerator,
                "{name} remainder"
            );
        } else {
            let ObjectRef::Ratio(remainder) = classify_object(&ctx, remainder) else {
                panic!("{name}: expected ratio remainder");
            };
            let remainder = ncl_object::Ratio::from_word(remainder);
            assert_eq!(
                integer(&ctx, ratio_numerator(&ctx, remainder).unwrap()),
                remainder_numerator,
                "{name} remainder numerator"
            );
            assert_eq!(
                integer(&ctx, ratio_denominator(&ctx, remainder).unwrap()),
                remainder_denominator,
                "{name} remainder denominator"
            );
        }
    }
}

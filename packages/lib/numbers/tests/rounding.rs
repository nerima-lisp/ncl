#![allow(clippy::unwrap_used, missing_docs)]

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
    for name in ["FCEILING", "FTRUNCATE", "FROUND"] {
        let quotient = call(&runtime, &mut ctx, name, &[value, divisor]).unwrap();
        assert!(matches!(
            classify_object(&ctx, quotient),
            ObjectRef::DoubleFloat(_)
        ));
        assert!(matches!(
            classify_object(&ctx, ctx.values()[1]),
            ObjectRef::DoubleFloat(_)
        ));
    }
}

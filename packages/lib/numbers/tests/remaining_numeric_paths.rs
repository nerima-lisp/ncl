#![allow(clippy::float_cmp, clippy::unwrap_used, missing_docs)]

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, complex_imag, complex_real, double_value, make_bignum_from_i128, make_complex,
    make_double, make_ratio, ratio_denominator, ratio_numerator,
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
    let symbol = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    let function = FunctionObject::try_from(symbol).unwrap();
    runtime.call_builtin(ctx, function, args)
}

fn integer(ctx: &ThreadContext, value: Word) -> i128 {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => i128::from(value),
        ObjectRef::Bignum(value) => {
            let value = ncl_object::Bignum::from_word(value);
            let magnitude = ncl_object::bignum_limbs(ctx, value)
                .unwrap()
                .into_iter()
                .enumerate()
                .fold(0_u128, |sum, (index, limb)| {
                    sum | (u128::from(limb) << (index * 32))
                });
            if ncl_object::bignum_sign(ctx, value).unwrap() {
                if magnitude == 1_u128 << 127 {
                    i128::MIN
                } else {
                    -i128::try_from(magnitude).unwrap()
                }
            } else {
                i128::try_from(magnitude).unwrap()
            }
        }
        other => panic!("expected integer, got {other:?}"),
    }
}

fn float(ctx: &ThreadContext, value: Word) -> f64 {
    let ObjectRef::DoubleFloat(value) = classify_object(ctx, value) else {
        panic!("expected double-float")
    };
    double_value(ctx, DoubleFloat::from_word(value)).unwrap()
}

fn bignum(ctx: &mut ThreadContext, runtime: &Runtime, value: i128) -> Word {
    make_bignum_from_i128(ctx, runtime, value).unwrap().into()
}

fn ratio(ctx: &mut ThreadContext, runtime: &Runtime, numerator: i128, denominator: i128) -> Word {
    let numerator = if let Ok(value) = i64::try_from(numerator) {
        Word::fixnum(value)
    } else {
        bignum(ctx, runtime, numerator)
    };
    let denominator = if let Ok(value) = i64::try_from(denominator) {
        Word::fixnum(value)
    } else {
        bignum(ctx, runtime, denominator)
    };
    make_ratio(ctx, runtime, numerator, denominator)
        .unwrap()
        .into()
}

fn complex(ctx: &mut ThreadContext, runtime: &Runtime, real: f64, imag: f64) -> Word {
    let real = make_double(ctx, runtime, real).unwrap().into();
    let imag = make_double(ctx, runtime, imag).unwrap().into();
    make_complex(ctx, runtime, real, imag).unwrap().into()
}

fn pair(ctx: &ThreadContext, value: Word) -> (f64, f64) {
    let ObjectRef::Complex(value) = classify_object(ctx, value) else {
        panic!("expected complex")
    };
    let value = ncl_object::Complex::from_word(value);
    (
        float(ctx, complex_real(ctx, value).unwrap()),
        float(ctx, complex_imag(ctx, value).unwrap()),
    )
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

fn call_float(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> f64 {
    let result = call(runtime, ctx, name, args).unwrap();
    float(ctx, result)
}

fn call_pair(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> (f64, f64) {
    let result = call(runtime, ctx, name, args).unwrap();
    pair(ctx, result)
}

fn assert_ratio(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    numerator: i128,
    denominator: i128,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    let ObjectRef::Ratio(result) = classify_object(ctx, result) else {
        panic!("{name}: expected ratio")
    };
    let result = ncl_object::Ratio::from_word(result);
    assert_eq!(
        integer(ctx, ratio_numerator(ctx, result).unwrap()),
        numerator
    );
    assert_eq!(
        integer(ctx, ratio_denominator(ctx, result).unwrap()),
        denominator
    );
}

#[test]
fn remaining_numeric_paths_cover_exact_ratio_operations_and_overflow_errors() {
    let (runtime, mut ctx) = setup();
    let one_third = ratio(&mut ctx, &runtime, 1, 3);
    let two_fifth = ratio(&mut ctx, &runtime, 2, 5);
    assert_ratio(&runtime, &mut ctx, "+", &[one_third, two_fifth], 11, 15);
    assert_ratio(&runtime, &mut ctx, "-", &[one_third, two_fifth], -1, 15);
    assert_ratio(&runtime, &mut ctx, "*", &[one_third, two_fifth], 2, 15);
    assert_ratio(&runtime, &mut ctx, "/", &[one_third, two_fifth], 5, 6);
    assert_ratio(
        &runtime,
        &mut ctx,
        "+",
        &[one_third, Word::fixnum(-2)],
        -5,
        3,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "-",
        &[Word::fixnum(-2), one_third],
        -7,
        3,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "*",
        &[one_third, Word::fixnum(-2)],
        -2,
        3,
    );
    assert_integer(&runtime, &mut ctx, "/", &[Word::fixnum(-2), one_third], -6);

    let max_over_two = ratio(&mut ctx, &runtime, i128::MAX, 2);
    let one_over_two = ratio(&mut ctx, &runtime, 1, 2);
    let two_over_three = ratio(&mut ctx, &runtime, 2, 3);
    for (name, args) in [
        ("+", [max_over_two, one_over_two]),
        ("+", [one_over_two, max_over_two]),
        ("*", [max_over_two, Word::fixnum(2)]),
        ("/", [bignum(&mut ctx, &runtime, i128::MAX), one_over_two]),
        ("/", [max_over_two, two_over_three]),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &args),
            Err(ObjectError::TypeError),
            "{name} overflow contract"
        );
    }

    assert_integer(
        &runtime,
        &mut ctx,
        "EXPT",
        &[Word::fixnum(-2), Word::fixnum(3)],
        -8,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "EXPT",
        &[Word::fixnum(-2), Word::fixnum(-3)],
        -1,
        8,
    );
    let fallback = call(
        &runtime,
        &mut ctx,
        "EXPT",
        &[Word::fixnum(2), Word::fixnum(127)],
    );
    assert!((float(&ctx, fallback.unwrap()) - 2_f64.powi(127)).abs() < 2_f64.powi(127) * 1e-12);
    assert_eq!(
        call(&runtime, &mut ctx, "EXPT", &[Word::TRUE, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn remaining_numeric_paths_cover_complex_components_and_division_contracts() {
    let (runtime, mut ctx) = setup();
    let z = complex(&mut ctx, &runtime, 3.0, -4.0);
    let w = complex(&mut ctx, &runtime, -2.0, 1.0);
    assert_eq!(call_pair(&runtime, &mut ctx, "+", &[z, w]), (1.0, -3.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "-", &[z, w]), (5.0, -5.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "*", &[z, w]), (-2.0, 11.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "/", &[z, w]), (-2.0, 1.0));
    assert_eq!(
        call_pair(&runtime, &mut ctx, "+", &[z, Word::fixnum(2)]),
        (5.0, -4.0)
    );
    assert_eq!(
        call_pair(&runtime, &mut ctx, "-", &[Word::fixnum(2), z]),
        (-1.0, 4.0)
    );
    assert_eq!(
        call_pair(&runtime, &mut ctx, "*", &[z, Word::fixnum(2)]),
        (6.0, -8.0)
    );
    assert_eq!(
        call_pair(&runtime, &mut ctx, "/", &[z, Word::fixnum(2)]),
        (1.5, 0.0)
    );
    assert_eq!(call_pair(&runtime, &mut ctx, "CONJUGATE", &[z]), (3.0, 4.0));
    assert_eq!(
        call(&runtime, &mut ctx, "CONJUGATE", &[Word::fixnum(3)]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(call_float(&runtime, &mut ctx, "REALPART", &[z]), 3.0);
    assert_eq!(call_float(&runtime, &mut ctx, "IMAGPART", &[z]), -4.0);
    assert_eq!(
        call_float(&runtime, &mut ctx, "REALPART", &[Word::fixnum(3)]),
        3.0
    );
    assert_eq!(
        call_float(&runtime, &mut ctx, "IMAGPART", &[Word::fixnum(3)]),
        0.0
    );
    assert_eq!(
        call_float(&runtime, &mut ctx, "PHASE", &[z]),
        (-4.0_f64).atan2(3.0)
    );
    assert_eq!(
        call_pair(&runtime, &mut ctx, "CIS", &[Word::fixnum(0)]),
        (1.0, 0.0)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMPLEX",
            &[Word::TRUE, Word::fixnum(1)]
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn remaining_numeric_paths_cover_remainder_signs_bignums_and_roots() {
    let (runtime, mut ctx) = setup();
    let value = bignum(&mut ctx, &runtime, (1_i128 << 100) + (1_i128 << 70));
    let divisor = bignum(&mut ctx, &runtime, 1_i128 << 100);
    assert_integer(&runtime, &mut ctx, "MOD", &[value, divisor], 1_i128 << 70);
    assert_integer(&runtime, &mut ctx, "REM", &[value, divisor], 1_i128 << 70);
    for (name, expected) in [("MOD", -2), ("REM", 1)] {
        assert_integer(
            &runtime,
            &mut ctx,
            name,
            &[Word::fixnum(7), Word::fixnum(-3)],
            expected,
        );
    }
    let negative = ratio(&mut ctx, &runtime, -7, 3);
    let positive_divisor = ratio(&mut ctx, &runtime, 2, 3);
    assert_ratio(
        &runtime,
        &mut ctx,
        "MOD",
        &[negative, positive_divisor],
        1,
        3,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "REM",
        &[negative, positive_divisor],
        -1,
        3,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "GCD",
        &[Word::fixnum(-84), Word::fixnum(30)],
        6,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(-6), Word::fixnum(15)],
        30,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(0), Word::fixnum(12)],
        0,
    );
    assert_integer(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(2)], 1);
    assert_integer(
        &runtime,
        &mut ctx,
        "ISQRT",
        &[Word::fixnum(1_i64 << 30)],
        1_i128 << 15,
    );
}

#[test]
fn remaining_numeric_paths_cover_all_rounding_modes_and_float_boundaries() {
    let (runtime, mut ctx) = setup();
    for (name, quotient, remainder) in [
        ("FLOOR", -4, 1),
        ("CEILING", -3, -1),
        ("TRUNCATE", -3, -1),
        ("ROUND", -4, 1),
    ] {
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
    for (name, expected) in [("FFLOOR", -8.0), ("FCEILING", -7.0), ("FTRUNCATE", -7.0)] {
        let value = make_double(&mut ctx, &runtime, -7.5).unwrap().into();
        let result = call(&runtime, &mut ctx, name, &[value]).unwrap();
        assert_eq!(float(&ctx, result), expected, "{name}");
    }
    for (value, expected_quotient, expected_remainder) in [
        (0.25, 0.0, 0.25),
        (0.5, 0.0, 0.5),
        (1.5, 2.0, -0.5),
        (2.5, 2.0, 0.5),
    ] {
        let value = make_double(&mut ctx, &runtime, value).unwrap().into();
        let result = call(&runtime, &mut ctx, "FROUND", &[value]).unwrap();
        assert_eq!(float(&ctx, result), expected_quotient);
        assert_eq!(float(&ctx, ctx.values()[1]), expected_remainder);
    }
    let huge = make_double(&mut ctx, &runtime, f64::MAX).unwrap().into();
    let result = call(&runtime, &mut ctx, "FROUND", &[huge]).unwrap();
    assert_eq!(float(&ctx, result), f64::MAX);
    assert_eq!(float(&ctx, ctx.values()[1]), 0.0);
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
            call(
                &runtime,
                &mut ctx,
                name,
                &[Word::fixnum(1), Word::fixnum(0)]
            ),
            Err(ObjectError::TypeError),
            "{name} zero divisor"
        );
    }
}

#[test]
fn remaining_numeric_paths_cover_random_state_and_limit_errors() {
    let (runtime, mut ctx) = setup();
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::TRUE]).unwrap();
    let clone = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[state]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(100), state]),
        call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(100), clone])
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(1), state]),
        Ok(Word::fixnum(0))
    );
    let large_limit = bignum(&mut ctx, &runtime, i128::MAX);
    let result = call(&runtime, &mut ctx, "RANDOM", &[large_limit, state]).unwrap();
    assert!(integer(&ctx, result) >= 0 && integer(&ctx, result) < i128::MAX);
    let float_limit = make_double(&mut ctx, &runtime, 3.5).unwrap().into();
    let float_result = call(&runtime, &mut ctx, "RANDOM", &[float_limit, state]).unwrap();
    assert!(float(&ctx, float_result) >= 0.0 && float(&ctx, float_result) < 3.5);
    for limit in [Word::NIL, Word::fixnum(0), Word::fixnum(-1)] {
        assert_eq!(
            call(&runtime, &mut ctx, "RANDOM", &[limit, state]),
            Err(ObjectError::TypeError)
        );
    }
    for limit in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        let limit = make_double(&mut ctx, &runtime, limit).unwrap().into();
        assert_eq!(
            call(&runtime, &mut ctx, "RANDOM", &[limit, state]),
            Err(ObjectError::TypeError)
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
}

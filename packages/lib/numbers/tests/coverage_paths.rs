#![allow(
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::unwrap_used,
    missing_docs
)]

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, complex_imag, complex_real, double_value, make_bignum_from_i128, make_complex,
    make_double, make_ratio,
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
    let symbol = match runtime.function(ctx, "COMMON-LISP", name) {
        Some(symbol) => symbol,
        None => panic!("missing builtin {name}"),
    };
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
        panic!("expected float")
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

fn call_integer(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> i128 {
    let result = call(runtime, ctx, name, args).unwrap();
    integer(ctx, result)
}

fn call_float(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> f64 {
    let result = call(runtime, ctx, name, args).unwrap();
    float(ctx, result)
}

fn call_pair(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> (f64, f64) {
    let result = call(runtime, ctx, name, args).unwrap();
    pair(ctx, result)
}

#[test]
fn arithmetic_covers_complex_float_ratio_and_empty_argument_paths() {
    let (runtime, mut ctx) = setup();
    let z = complex(&mut ctx, &runtime, 3.0, 4.0);
    let w = complex(&mut ctx, &runtime, 1.0, -2.0);
    let real = make_double(&mut ctx, &runtime, 2.0).unwrap().into();

    assert_eq!(call_pair(&runtime, &mut ctx, "+", &[z, w]), (4.0, 2.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "+", &[z, real]), (5.0, 4.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "+", &[real, z]), (5.0, 4.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "-", &[z, w]), (2.0, 6.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "-", &[z, real]), (1.0, 4.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "-", &[real, z]), (-1.0, -4.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "*", &[z, w]), (11.0, -2.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "*", &[z, real]), (6.0, 8.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "*", &[real, z]), (6.0, 8.0));
    assert_eq!(call_pair(&runtime, &mut ctx, "/", &[z, w]), (-1.0, 2.0));

    let abs_result = call_float(&runtime, &mut ctx, "ABS", &[real]);
    assert_eq!(abs_result, 2.0);
    assert_eq!(call_float(&runtime, &mut ctx, "SIGNUM", &[real]), 1.0);
    assert_eq!(
        call_integer(&runtime, &mut ctx, "SIGNUM", &[Word::fixnum(-9)]),
        -1
    );
    assert_eq!(call_pair(&runtime, &mut ctx, "SIGNUM", &[z]), (0.6, 0.8));
    assert_eq!(call(&runtime, &mut ctx, "+", &[]), Ok(Word::fixnum(0)));
    assert_eq!(
        call(&runtime, &mut ctx, "-", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(call(&runtime, &mut ctx, "*", &[]), Ok(Word::fixnum(1)));
    assert_eq!(
        call(&runtime, &mut ctx, "/", &[]),
        Err(ObjectError::TypeError)
    );

    let small = ratio(&mut ctx, &runtime, 1, 3);
    assert_eq!(
        call_float(&runtime, &mut ctx, "+", &[small, real]),
        2.3333333333333335
    );
    let huge_denominator = ratio(&mut ctx, &runtime, 1, 1_i128 << 100);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "+",
            &[huge_denominator, huge_denominator]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "*",
            &[huge_denominator, huge_denominator]
        ),
        Err(ObjectError::TypeError)
    );
    let huge_numerator = ratio(&mut ctx, &runtime, 1_i128 << 100, 3);
    assert_eq!(
        call(&runtime, &mut ctx, "/", &[huge_denominator, huge_numerator],),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn arithmetic_comparison_and_predicate_negative_paths_are_observable() {
    let (runtime, mut ctx) = setup();
    let ratio_value = ratio(&mut ctx, &runtime, 3, 2);
    let float_value = make_double(&mut ctx, &runtime, 1.5).unwrap().into();
    let complex_value = complex(&mut ctx, &runtime, 1.0, 2.0);

    assert_eq!(
        call(&runtime, &mut ctx, "EQ", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQL", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQL", &[Word::fixnum(1), ratio_value]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQL", &[float_value, float_value]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQL", &[complex_value, complex_value]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "EQL", &[Word::fixnum(1), Word::TRUE]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "MIN",
            &[Word::fixnum(2), Word::fixnum(1)]
        ),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MAX", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MIN", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "PLUSP", &[complex_value]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MINUSP", &[complex_value]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(call_float(&runtime, &mut ctx, "ABS", &[float_value]), 1.5);
}

#[test]
fn boole_and_integer_bitops_cover_all_opcodes_boundaries_and_errors() {
    let (runtime, mut ctx) = setup();
    let a = Word::fixnum(0b1100);
    let b = Word::fixnum(0b1010);
    let expected = [
        0, -15, -15, -13, 4, -11, 6, -9, 8, -7, 12, -3, 10, -5, 14, -1,
    ];
    for (opcode, expected) in expected.into_iter().enumerate() {
        assert_integer(
            &runtime,
            &mut ctx,
            "BOOLE",
            &[Word::fixnum(opcode as i64), a, b],
            expected,
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "BOOLE", &[a, b]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BOOLE", &[Word::fixnum(99), a, b]),
        Err(ObjectError::TypeError)
    );

    assert_eq!(
        call(&runtime, &mut ctx, "LOGTEST", &[a, Word::fixnum(2)]).unwrap(),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "LOGBITP",
            &[Word::fixnum(2), Word::fixnum(1)]
        )
        .unwrap(),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LOGBITP", &[Word::fixnum(-1), a]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "LOGBITP",
            &[Word::fixnum(127), Word::fixnum(1)]
        )
        .unwrap(),
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LOGCOUNT", &[Word::fixnum(9)]).unwrap(),
        Word::fixnum(2)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "INTEGER-LENGTH", &[Word::fixnum(9)]).unwrap(),
        Word::fixnum(4)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ASH",
            &[Word::fixnum(1), Word::fixnum(128)]
        ),
        Err(ObjectError::Layout)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ASH",
            &[Word::fixnum(1), Word::fixnum(-128)]
        ),
        Err(ObjectError::Layout)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LOGNOT", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LOGANDC1", &[a]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LOGORC2", &[a]),
        Err(ObjectError::TypeError)
    );

    let large = bignum(&mut ctx, &runtime, 1_i128 << 70);
    assert_integer(
        &runtime,
        &mut ctx,
        "LOGIOR",
        &[large, Word::fixnum(1)],
        (1_i128 << 70) + 1,
    );
    assert_integer(&runtime, &mut ctx, "LOGNOT", &[large], -(1_i128 << 70) - 1);
    assert_eq!(
        call(&runtime, &mut ctx, "LOGAND", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn byte_fields_cover_false_values_wide_fields_and_invalid_specs() {
    let (runtime, mut ctx) = setup();
    let wide = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(127), Word::fixnum(0)],
    )
    .unwrap();
    assert_integer(
        &runtime,
        &mut ctx,
        "LDB",
        &[wide, Word::fixnum(-1)],
        i128::MAX,
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LDB-TEST", &[wide, Word::fixnum(0)]).unwrap(),
        Word::NIL
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "MASK-FIELD",
        &[wide, Word::fixnum(-1)],
        i128::MAX,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "DPB",
        &[Word::fixnum(0), wide, Word::fixnum(-1)],
        i128::MIN,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "DEPOSIT-FIELD",
        &[Word::fixnum(0), wide, Word::fixnum(-1)],
        i128::MIN,
    );

    let invalid_size = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(128), Word::fixnum(0)],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "LDB", &[invalid_size, Word::fixnum(1)]),
        Err(ObjectError::Layout)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BYTE-SIZE", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BYTE-POSITION", &[]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DPB", &[Word::fixnum(1), wide]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "DEPOSIT-FIELD",
            &[Word::fixnum(1), wide]
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn rounding_exercises_float_conversion_boundaries_and_remainder_types() {
    let (runtime, mut ctx) = setup();
    for (value, expected, remainder) in [(2.5, 2.0, 0.5), (3.5, 4.0, -0.5), (-2.5, -2.0, -0.5)] {
        let value = make_double(&mut ctx, &runtime, value).unwrap().into();
        let result = call(&runtime, &mut ctx, "FROUND", &[value]).unwrap();
        assert_eq!(float(&ctx, result), expected);
        assert_eq!(float(&ctx, ctx.values()[1]), remainder);
    }
    for (value, expected) in [
        (f64::INFINITY, f64::INFINITY),
        (f64::MAX, f64::MAX),
        (f64::from_bits(1), 0.0),
    ] {
        let value = make_double(&mut ctx, &runtime, value).unwrap().into();
        let result = call(&runtime, &mut ctx, "FROUND", &[value]).unwrap();
        assert_eq!(float(&ctx, result).to_bits(), expected.to_bits());
    }
    let one = Word::fixnum(1);
    let value = make_double(&mut ctx, &runtime, 2.75).unwrap().into();
    let result = call(&runtime, &mut ctx, "FLOOR", &[value, one]).unwrap();
    assert_eq!(float(&ctx, result), 2.0);
    assert_eq!(float(&ctx, ctx.values()[1]), 0.75);
    assert_eq!(
        call(&runtime, &mut ctx, "ROUND", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn transcendental_paths_accept_bignums_ratios_and_complex_arguments() {
    let (runtime, mut ctx) = setup();
    let huge = bignum(&mut ctx, &runtime, 1_i128 << 70);
    let third = ratio(&mut ctx, &runtime, 1, 3);
    let z = complex(&mut ctx, &runtime, 0.3, -0.7);

    assert!((call_float(&runtime, &mut ctx, "SQRT", &[huge]) - (2_f64.powi(35))).abs() < 1.0);
    assert!(
        (call_float(&runtime, &mut ctx, "SIN", &[third]) - (1.0_f64 / 3.0).sin()).abs() < 1e-12
    );
    let log_result = call_pair(&runtime, &mut ctx, "LOG", &[z]);
    assert!((log_result.0 - 0.3_f64.hypot(-0.7).ln()).abs() < 1e-12);
    assert!((log_result.1 - (-0.7_f64).atan2(0.3)).abs() < 1e-12);
    let expt_result = call_pair(&runtime, &mut ctx, "EXPT", &[z, Word::fixnum(2)]);
    assert!((expt_result.0 - (-0.4)).abs() < 1e-12);
    assert!((expt_result.1 - (-0.42)).abs() < 1e-12);
    let half = make_double(&mut ctx, &runtime, 0.5).unwrap().into();
    let expt_half = call_pair(&runtime, &mut ctx, "EXPT", &[Word::fixnum(-1), half]);
    assert!(expt_half.0.abs() < 1e-12);
    assert_eq!(expt_half.1, 1.0);
    assert_eq!(
        call(&runtime, &mut ctx, "EXP", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LOG", &[Word::fixnum(1), Word::TRUE]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ATAN", &[Word::fixnum(1), Word::TRUE]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn remainder_covers_float_modulo_overflow_and_large_integer_roots() {
    let (runtime, mut ctx) = setup();
    let value = make_double(&mut ctx, &runtime, -7.5).unwrap().into();
    let divisor = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    assert_eq!(
        call_float(&runtime, &mut ctx, "MOD", &[value, divisor]),
        0.5
    );
    assert_eq!(
        call_float(&runtime, &mut ctx, "REM", &[value, Word::fixnum(2)]),
        -1.5
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MOD", &[Word::TRUE, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
    let sqrt_input = bignum(&mut ctx, &runtime, 1_i128 << 100);
    assert_integer(&runtime, &mut ctx, "ISQRT", &[sqrt_input], 1_i128 << 50);
    let gcd_input = bignum(&mut ctx, &runtime, 1_i128 << 70);
    assert_integer(&runtime, &mut ctx, "GCD", &[gcd_input, Word::fixnum(3)], 1);
    let min_input = bignum(&mut ctx, &runtime, i128::MIN);
    assert_eq!(
        call(&runtime, &mut ctx, "LCM", &[min_input, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "GCD", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "ISQRT", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );
}

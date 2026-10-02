#![allow(
    clippy::float_cmp,
    clippy::map_unwrap_or,
    clippy::unwrap_used,
    missing_docs
)]

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, double_value, make_bignum_from_i128, make_double, make_ratio,
    ratio_denominator, ratio_numerator,
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
    let symbol = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
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
    let numerator = i64::try_from(numerator)
        .map(Word::fixnum)
        .unwrap_or_else(|_| bignum(ctx, runtime, numerator));
    let denominator = i64::try_from(denominator)
        .map(Word::fixnum)
        .unwrap_or_else(|_| bignum(ctx, runtime, denominator));
    make_ratio(ctx, runtime, numerator, denominator)
        .unwrap()
        .into()
}

fn assert_ratio(ctx: &ThreadContext, value: Word, numerator: i128, denominator: i128) {
    let ObjectRef::Ratio(value) = classify_object(ctx, value) else {
        panic!("expected ratio")
    };
    let value = ncl_object::Ratio::from_word(value);
    assert_eq!(
        integer(ctx, ratio_numerator(ctx, value).unwrap()),
        numerator
    );
    assert_eq!(
        integer(ctx, ratio_denominator(ctx, value).unwrap()),
        denominator
    );
}

fn assert_float_bits(ctx: &ThreadContext, value: Word, expected: f64) {
    assert_eq!(float(ctx, value).to_bits(), expected.to_bits());
}

#[test]
fn rational_covers_integer_ratio_float_and_nonfinite_inputs() {
    let (runtime, mut ctx) = setup();
    assert_eq!(
        call(&runtime, &mut ctx, "RATIONAL", &[Word::fixnum(-7)]).unwrap(),
        Word::fixnum(-7)
    );

    let large_integer = bignum(&mut ctx, &runtime, 1_i128 << 70);
    let result = call(&runtime, &mut ctx, "RATIONAL", &[large_integer]).unwrap();
    assert_eq!(integer(&ctx, result), 1_i128 << 70);

    let negative_ratio = ratio(&mut ctx, &runtime, -6, -4);
    let result = call(&runtime, &mut ctx, "RATIONAL", &[negative_ratio]).unwrap();
    assert_ratio(&ctx, result, 3, 2);

    let quarter = make_double(&mut ctx, &runtime, 0.25).unwrap().into();
    let result = call(&runtime, &mut ctx, "RATIONAL", &[quarter]).unwrap();
    assert_ratio(&ctx, result, 1, 4);

    let infinity = make_double(&mut ctx, &runtime, f64::INFINITY)
        .unwrap()
        .into();
    assert_eq!(
        call(&runtime, &mut ctx, "RATIONAL", &[infinity]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RATIONAL", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn float_sign_digits_radix_and_precision_return_exact_contract_values() {
    let (runtime, mut ctx) = setup();
    let negative_zero = make_double(&mut ctx, &runtime, -0.0).unwrap().into();
    let positive = make_double(&mut ctx, &runtime, 3.0).unwrap().into();
    let negative_scale = make_double(&mut ctx, &runtime, -2.5).unwrap().into();

    let result = call(&runtime, &mut ctx, "FLOAT-SIGN", &[negative_zero]).unwrap();
    assert_float_bits(&ctx, result, -1.0);
    let result = call(
        &runtime,
        &mut ctx,
        "FLOAT-SIGN",
        &[positive, negative_scale],
    )
    .unwrap();
    assert_float_bits(&ctx, result, 2.5);
    assert_eq!(
        call(&runtime, &mut ctx, "FLOAT-DIGITS", &[positive]).unwrap(),
        Word::fixnum(53)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FLOAT-RADIX", &[positive]).unwrap(),
        Word::fixnum(2)
    );

    let normal = make_double(&mut ctx, &runtime, 1.0).unwrap().into();
    assert_eq!(
        call(&runtime, &mut ctx, "FLOAT-PRECISION", &[normal]).unwrap(),
        Word::fixnum(53)
    );
    let subnormal = make_double(&mut ctx, &runtime, f64::from_bits(0x000f))
        .unwrap()
        .into();
    assert_eq!(
        call(&runtime, &mut ctx, "FLOAT-PRECISION", &[subnormal]).unwrap(),
        Word::fixnum(4)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FLOAT-RADIX", &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn scale_float_covers_signed_normal_and_clamped_integer_exponents() {
    let (runtime, mut ctx) = setup();
    let value = make_double(&mut ctx, &runtime, 1.5).unwrap().into();

    let result = call(
        &runtime,
        &mut ctx,
        "SCALE-FLOAT",
        &[value, Word::fixnum(-2)],
    )
    .unwrap();
    assert_float_bits(&ctx, result, 0.375);
    let minimum_scale = bignum(&mut ctx, &runtime, i128::MIN);
    let result = call(&runtime, &mut ctx, "SCALE-FLOAT", &[value, minimum_scale]).unwrap();
    assert_float_bits(&ctx, result, 0.0);
    let maximum_scale = bignum(&mut ctx, &runtime, i128::MAX);
    let result = call(&runtime, &mut ctx, "SCALE-FLOAT", &[value, maximum_scale]).unwrap();
    assert_float_bits(&ctx, result, f64::INFINITY);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SCALE-FLOAT",
            &[Word::fixnum(1), Word::fixnum(1)],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn decode_float_builtins_report_subnormal_significands_and_exponents() {
    let (runtime, mut ctx) = setup();
    let smallest = make_double(&mut ctx, &runtime, f64::from_bits(1))
        .unwrap()
        .into();
    let negative_smallest = make_double(&mut ctx, &runtime, -f64::from_bits(1))
        .unwrap()
        .into();

    let significand = call(&runtime, &mut ctx, "DECODE-FLOAT", &[smallest]).unwrap();
    assert_float_bits(&ctx, significand, 2_f64.powi(-52));
    assert_eq!(integer(&ctx, ctx.values()[1]), -1022);
    assert_float_bits(&ctx, ctx.values()[2], 1.0);

    let significand = call(&runtime, &mut ctx, "DECODE-FLOAT", &[negative_smallest]).unwrap();
    assert_float_bits(&ctx, significand, 2_f64.powi(-52));
    assert_eq!(integer(&ctx, ctx.values()[1]), -1022);
    assert_float_bits(&ctx, ctx.values()[2], -1.0);

    let significand = call(&runtime, &mut ctx, "INTEGER-DECODE-FLOAT", &[smallest]).unwrap();
    assert_eq!(integer(&ctx, significand), 1);
    assert_eq!(integer(&ctx, ctx.values()[1]), -1074);
    assert_eq!(integer(&ctx, ctx.values()[2]), 1);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "INTEGER-DECODE-FLOAT",
            &[Word::fixnum(1)],
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn rationalize_covers_tolerance_continued_fractions_and_invalid_values() {
    let (runtime, mut ctx) = setup();
    let positive = make_double(&mut ctx, &runtime, 0.3).unwrap().into();
    let negative = make_double(&mut ctx, &runtime, -0.3).unwrap().into();
    let tolerance = make_double(&mut ctx, &runtime, 0.05).unwrap().into();

    let result = call(&runtime, &mut ctx, "RATIONALIZE", &[positive, tolerance]).unwrap();
    assert_ratio(&ctx, result, 1, 3);
    let result = call(&runtime, &mut ctx, "RATIONALIZE", &[negative, tolerance]).unwrap();
    assert_ratio(&ctx, result, -1, 3);
    let result = call(
        &runtime,
        &mut ctx,
        "RATIONALIZE",
        &[Word::fixnum(4), tolerance],
    )
    .unwrap();
    assert_eq!(result, Word::fixnum(4));

    let negative_tolerance = make_double(&mut ctx, &runtime, -0.01).unwrap().into();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "RATIONALIZE",
            &[positive, negative_tolerance]
        ),
        Err(ObjectError::TypeError)
    );
    let infinite_tolerance = make_double(&mut ctx, &runtime, f64::INFINITY)
        .unwrap()
        .into();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "RATIONALIZE",
            &[positive, infinite_tolerance]
        ),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "RATIONALIZE",
            &[positive, Word::fixnum(0)]
        ),
        Err(ObjectError::TypeError)
    );
    let huge_tolerance = make_double(&mut ctx, &runtime, f64::MAX).unwrap().into();
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "RATIONALIZE",
            &[positive, huge_tolerance]
        ),
        Err(ObjectError::TypeError)
    );
    let infinity = make_double(&mut ctx, &runtime, f64::INFINITY)
        .unwrap()
        .into();
    assert_eq!(
        call(&runtime, &mut ctx, "RATIONALIZE", &[infinity]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn rationalize_covers_zero_ratio_and_large_integer_results() {
    let (runtime, mut ctx) = setup();
    let zero = make_double(&mut ctx, &runtime, 0.0).unwrap().into();
    let result = call(&runtime, &mut ctx, "RATIONALIZE", &[zero]).unwrap();
    assert_eq!(result, Word::fixnum(0));

    let zero_ratio = ratio(&mut ctx, &runtime, 0, 7);
    let result = call(&runtime, &mut ctx, "RATIONAL", &[zero_ratio]).unwrap();
    assert_eq!(result, Word::fixnum(0));

    let existing_ratio = ratio(&mut ctx, &runtime, 6, 4);
    let result = call(&runtime, &mut ctx, "RATIONALIZE", &[existing_ratio]).unwrap();
    assert_ratio(&ctx, result, 3, 2);

    let large_float = make_double(&mut ctx, &runtime, 2_f64.powi(60))
        .unwrap()
        .into();
    let zero_tolerance = make_double(&mut ctx, &runtime, 0.0).unwrap().into();
    let result = call(
        &runtime,
        &mut ctx,
        "RATIONALIZE",
        &[large_float, zero_tolerance],
    )
    .unwrap();
    assert_eq!(integer(&ctx, result), 1_i128 << 60);

    let crossing_zero = make_double(&mut ctx, &runtime, 0.3).unwrap().into();
    let wide_tolerance = make_double(&mut ctx, &runtime, 1.5).unwrap().into();
    let result = call(
        &runtime,
        &mut ctx,
        "RATIONALIZE",
        &[crossing_zero, wide_tolerance],
    )
    .unwrap();
    assert_eq!(result, Word::fixnum(-1));
}

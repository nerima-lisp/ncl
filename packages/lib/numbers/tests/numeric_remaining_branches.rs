#![allow(
    clippy::float_cmp,
    clippy::cast_lossless,
    clippy::map_unwrap_or,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::unreadable_literal,
    missing_docs,
    reason = "tests assert concrete numeric builtin contracts"
)]

use ncl_object::{
    Bignum, DoubleFloat, FunctionObject, ObjectError, ObjectRef, Package, Runtime, ThreadContext,
    Word, bignum_limbs, bignum_sign, classify_object, complex_imag, complex_real, double_value,
    make_bignum_from_i128, make_double, make_instance, make_ratio, ratio_denominator,
    ratio_numerator, symbol_value,
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
            let value = Bignum::from_word(value);
            let magnitude = bignum_limbs(ctx, value)
                .unwrap()
                .into_iter()
                .enumerate()
                .fold(0_u128, |sum, (index, limb)| {
                    sum | (u128::from(limb) << (index * 32))
                });
            if bignum_sign(ctx, value).unwrap() {
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
        panic!(
            "expected double float, got {:?}",
            classify_object(ctx, value)
        );
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

fn pair(ctx: &ThreadContext, value: Word) -> (f64, f64) {
    let ObjectRef::Complex(value) = classify_object(ctx, value) else {
        panic!("expected complex, got {:?}", classify_object(ctx, value));
    };
    let value = ncl_object::Complex::from_word(value);
    (
        float(ctx, complex_real(ctx, value).unwrap()),
        float(ctx, complex_imag(ctx, value).unwrap()),
    )
}

fn common_lisp_symbol(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
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

fn assert_float(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: f64,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    assert_eq!(float(ctx, result), expected, "{name}");
}

fn assert_ratio(
    ctx: &ThreadContext,
    value: Word,
    expected_numerator: i128,
    expected_denominator: i128,
) {
    let ObjectRef::Ratio(value) = classify_object(ctx, value) else {
        panic!("expected ratio");
    };
    let value = ncl_object::Ratio::from_word(value);
    assert_eq!(
        integer(ctx, ratio_numerator(ctx, value).unwrap()),
        expected_numerator
    );
    assert_eq!(
        integer(ctx, ratio_denominator(ctx, value).unwrap()),
        expected_denominator
    );
}

fn assert_type_error(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) {
    assert_eq!(
        call(runtime, ctx, name, args),
        Err(ObjectError::TypeError),
        "{name}"
    );
}

#[test]
fn rational_float_matrix_covers_negative_bignums_optional_float_and_bad_operands() {
    let (runtime, mut ctx) = setup();
    let negative_large = bignum(&mut ctx, &runtime, -(1_i128 << 72) - 3);
    let rational = call(&runtime, &mut ctx, "RATIONAL", &[negative_large]).unwrap();
    assert_eq!(integer(&ctx, rational), -(1_i128 << 72) - 3);

    let negative_one_point_five = make_double(&mut ctx, &runtime, -1.5).unwrap().into();
    let rational = call(&runtime, &mut ctx, "RATIONAL", &[negative_one_point_five]).unwrap();
    assert_ratio(&ctx, rational, -3, 2);
    let negative_zero = make_double(&mut ctx, &runtime, -0.0).unwrap().into();
    assert_eq!(
        call(&runtime, &mut ctx, "RATIONAL", &[negative_zero]),
        Err(ObjectError::TypeError)
    );

    let negative_ratio = ratio(&mut ctx, &runtime, -14, 9);
    assert_integer(&runtime, &mut ctx, "NUMERATOR", &[negative_ratio], -14);
    assert_integer(&runtime, &mut ctx, "DENOMINATOR", &[negative_ratio], 9);
    let negative_denominator = ratio(&mut ctx, &runtime, -6, -4);
    let normalized = call(&runtime, &mut ctx, "RATIONAL", &[negative_denominator]).unwrap();
    assert_ratio(&ctx, normalized, 3, 2);
    let float_value = make_double(&mut ctx, &runtime, 4.25).unwrap().into();
    assert_type_error(&runtime, &mut ctx, "NUMERATOR", &[float_value]);
    assert_type_error(&runtime, &mut ctx, "DENOMINATOR", &[float_value]);
    assert_type_error(&runtime, &mut ctx, "NUMERATOR", &[Word::TRUE]);
    assert_type_error(&runtime, &mut ctx, "DENOMINATOR", &[Word::NIL]);

    assert_float(&runtime, &mut ctx, "FLOAT", &[Word::fixnum(-13)], -13.0);
    let ratio_value = ratio(&mut ctx, &runtime, 7, 4);
    assert_float(&runtime, &mut ctx, "FLOAT", &[ratio_value], 1.75);
    let format = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    assert_float(
        &runtime,
        &mut ctx,
        "FLOAT",
        &[Word::fixnum(17), format],
        17.0,
    );
    assert_type_error(
        &runtime,
        &mut ctx,
        "FLOAT",
        &[Word::fixnum(17), Word::fixnum(2)],
    );
    assert_type_error(&runtime, &mut ctx, "FLOAT", &[Word::TRUE]);

    let positive = make_double(&mut ctx, &runtime, 9.0).unwrap().into();
    let negative_sign = make_double(&mut ctx, &runtime, -3.0).unwrap().into();
    assert_float(
        &runtime,
        &mut ctx,
        "FLOAT-SIGN",
        &[positive, negative_sign],
        3.0,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "FLOAT-SIGN",
        &[negative_sign, positive],
        -9.0,
    );
    assert_type_error(
        &runtime,
        &mut ctx,
        "FLOAT-SIGN",
        &[positive, Word::fixnum(1)],
    );
    assert_integer(&runtime, &mut ctx, "FLOAT-PRECISION", &[negative_zero], 0);
    assert_integer(&runtime, &mut ctx, "FLOAT-DIGITS", &[negative_sign], 53);
    assert_integer(&runtime, &mut ctx, "FLOAT-RADIX", &[negative_sign], 2);
}

#[test]
fn rational_float_matrix_covers_decode_normal_paths_scale_and_exact_tolerance() {
    let (runtime, mut ctx) = setup();
    let six_point_five = make_double(&mut ctx, &runtime, 6.5).unwrap().into();
    let significand = call(&runtime, &mut ctx, "DECODE-FLOAT", &[six_point_five]).unwrap();
    assert_eq!(float(&ctx, significand), 0.8125);
    assert_eq!(integer(&ctx, ctx.values()[1]), 3);
    assert_eq!(float(&ctx, ctx.values()[2]), 1.0);
    let integer_significand = call(
        &runtime,
        &mut ctx,
        "INTEGER-DECODE-FLOAT",
        &[six_point_five],
    )
    .unwrap();
    assert_eq!(integer(&ctx, integer_significand), 7_318_349_394_477_056);
    assert_eq!(integer(&ctx, ctx.values()[1]), -50);
    assert_eq!(integer(&ctx, ctx.values()[2]), 1);

    let negative = make_double(&mut ctx, &runtime, -6.5).unwrap().into();
    let decoded = call(&runtime, &mut ctx, "INTEGER-DECODE-FLOAT", &[negative]).unwrap();
    assert_eq!(integer(&ctx, decoded), 7_318_349_394_477_056);
    assert_eq!(integer(&ctx, ctx.values()[1]), -50);
    assert_eq!(integer(&ctx, ctx.values()[2]), -1);

    let value = make_double(&mut ctx, &runtime, 1.25).unwrap().into();
    assert_float(
        &runtime,
        &mut ctx,
        "SCALE-FLOAT",
        &[value, Word::fixnum(3)],
        10.0,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "SCALE-FLOAT",
        &[value, Word::fixnum(0)],
        1.25,
    );
    assert_type_error(
        &runtime,
        &mut ctx,
        "SCALE-FLOAT",
        &[Word::fixnum(5), Word::fixnum(3)],
    );

    let three_eighths = make_double(&mut ctx, &runtime, 0.375).unwrap().into();
    let zero_tolerance = make_double(&mut ctx, &runtime, 0.0).unwrap().into();
    let result = call(
        &runtime,
        &mut ctx,
        "RATIONALIZE",
        &[three_eighths, zero_tolerance],
    )
    .unwrap();
    assert_ratio(&ctx, result, 1_688_849_860_263_938, 4_503_599_627_370_501);
    let tolerance = make_double(&mut ctx, &runtime, 0.01).unwrap().into();
    let result = call(
        &runtime,
        &mut ctx,
        "RATIONALIZE",
        &[three_eighths, tolerance],
    )
    .unwrap();
    assert_ratio(&ctx, result, 3, 8);
    let negative = make_double(&mut ctx, &runtime, -0.375).unwrap().into();
    let result = call(&runtime, &mut ctx, "RATIONALIZE", &[negative, tolerance]).unwrap();
    assert_ratio(&ctx, result, -3, 8);
    assert_type_error(
        &runtime,
        &mut ctx,
        "RATIONALIZE",
        &[three_eighths, Word::fixnum(1)],
    );
}

#[test]
fn random_matrix_asserts_seeded_integer_float_and_bignum_results() {
    let (runtime, mut ctx) = setup();
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[state]),
        Ok(Word::TRUE)
    );
    assert_integer(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(10), state], 2);
    assert_integer(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(10), state], 3);
    assert_integer(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(1), state], 0);

    let wide_state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    let wide_limit = bignum(&mut ctx, &runtime, 1_i128 << 100);
    let wide_result = call(&runtime, &mut ctx, "RANDOM", &[wide_limit, wide_state]).unwrap();
    assert_eq!(
        integer(&ctx, wide_result),
        180_583_166_191_023_279_029_129_237_250
    );
    let float_state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    let float_limit = make_double(&mut ctx, &runtime, 3.5).unwrap().into();
    let float_result = call(&runtime, &mut ctx, "RANDOM", &[float_limit, float_state]).unwrap();
    let expected = 3.5 * 3_337_323_988_657_558_274.0 / 4_611_686_018_427_387_904.0;
    assert!((float(&ctx, float_result) - expected).abs() < 1e-15);

    let true_state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::TRUE]).unwrap();
    let true_clone = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[true_state]).unwrap();
    let first = call(
        &runtime,
        &mut ctx,
        "RANDOM",
        &[Word::fixnum(97), true_state],
    )
    .unwrap();
    let second = call(
        &runtime,
        &mut ctx,
        "RANDOM",
        &[Word::fixnum(97), true_clone],
    )
    .unwrap();
    assert_eq!(integer(&ctx, first), integer(&ctx, second));
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[Word::TRUE]),
        Ok(Word::NIL)
    );
    assert_type_error(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(9), Word::TRUE]);
    let negative_limit = bignum(&mut ctx, &runtime, -1);
    assert_type_error(&runtime, &mut ctx, "RANDOM", &[negative_limit, state]);
}

#[test]
fn rounding_matrix_covers_exact_integer_remainders_ratio_float_fallbacks_and_large_ties() {
    let (runtime, mut ctx) = setup();
    let exact = ratio(&mut ctx, &runtime, 12, 3);
    let quotient = call(&runtime, &mut ctx, "FLOOR", &[exact, Word::fixnum(1)]).unwrap();
    assert_eq!(integer(&ctx, quotient), 4);
    assert_eq!(integer(&ctx, ctx.values()[1]), 0);

    let ratio_value = ratio(&mut ctx, &runtime, 7, 3);
    let float_divisor = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    let quotient = call(&runtime, &mut ctx, "FLOOR", &[ratio_value, float_divisor]).unwrap();
    assert_eq!(float(&ctx, quotient), 1.0);
    assert_eq!(float(&ctx, ctx.values()[1]), 0.333_333_333_333_333_5);

    let exact_value = Word::fixnum(7);
    let ratio_divisor = ratio(&mut ctx, &runtime, 3, 2);
    let quotient = call(
        &runtime,
        &mut ctx,
        "FCEILING",
        &[exact_value, ratio_divisor],
    )
    .unwrap();
    assert_eq!(float(&ctx, quotient), 5.0);
    assert_ratio(&ctx, ctx.values()[1], -1, 2);

    let quarter = make_double(&mut ctx, &runtime, 0.25).unwrap().into();
    let rounded = call(&runtime, &mut ctx, "FROUND", &[quarter]).unwrap();
    assert_eq!(float(&ctx, rounded), 0.0);
    assert_eq!(float(&ctx, ctx.values()[1]), 0.25);
    let tie = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    let rounded = call(&runtime, &mut ctx, "FROUND", &[tie]).unwrap();
    assert_eq!(float(&ctx, rounded), 2.0);
    assert_eq!(float(&ctx, ctx.values()[1]), 0.5);
    let odd_tie = make_double(&mut ctx, &runtime, 3.5).unwrap().into();
    let rounded = call(&runtime, &mut ctx, "FROUND", &[odd_tie]).unwrap();
    assert_eq!(float(&ctx, rounded), 4.0);
    assert_eq!(float(&ctx, ctx.values()[1]), -0.5);

    let huge = make_double(&mut ctx, &runtime, -2_f64.powi(127))
        .unwrap()
        .into();
    let rounded = call(&runtime, &mut ctx, "FROUND", &[huge]).unwrap();
    assert_eq!(float(&ctx, rounded), -2_f64.powi(127));
    assert_eq!(float(&ctx, ctx.values()[1]), 0.0);
    let nan = make_double(&mut ctx, &runtime, f64::NAN).unwrap().into();
    let result = call(&runtime, &mut ctx, "FROUND", &[nan]).unwrap();
    assert!(float(&ctx, result).is_nan());
    assert!(float(&ctx, ctx.values()[1]).is_nan());
}

#[test]
fn complex_matrix_covers_real_components_zero_imaginary_and_phase_contracts() {
    let (runtime, mut ctx) = setup();
    let real = Word::fixnum(11);
    let zero = Word::fixnum(0);
    assert_eq!(call(&runtime, &mut ctx, "COMPLEX", &[real, zero]), Ok(real));
    let imaginary = Word::fixnum(-4);
    let z = call(&runtime, &mut ctx, "COMPLEX", &[real, imaginary]).unwrap();
    assert_eq!(pair(&ctx, z), (11.0, -4.0));
    let float_real = make_double(&mut ctx, &runtime, -2.5).unwrap().into();
    let float_imag = make_double(&mut ctx, &runtime, 0.75).unwrap().into();
    let w = call(&runtime, &mut ctx, "COMPLEX", &[float_real, float_imag]).unwrap();
    assert_eq!(pair(&ctx, w), (-2.5, 0.75));
    let conjugate = call(&runtime, &mut ctx, "CONJUGATE", &[w]).unwrap();
    assert_eq!(pair(&ctx, conjugate), (-2.5, -0.75));
    assert_float(&runtime, &mut ctx, "REALPART", &[w], -2.5);
    assert_float(&runtime, &mut ctx, "IMAGPART", &[w], 0.75);
    assert_float(
        &runtime,
        &mut ctx,
        "PHASE",
        &[Word::fixnum(-1)],
        std::f64::consts::PI,
    );
    let cis_angle = make_double(&mut ctx, &runtime, std::f64::consts::FRAC_PI_2)
        .unwrap()
        .into();
    let cis = call(&runtime, &mut ctx, "CIS", &[cis_angle]).unwrap();
    let (cis_real, cis_imag) = pair(&ctx, cis);
    assert!(cis_real.abs() < 1e-15);
    assert!((cis_imag - 1.0).abs() < 1e-15);
    assert_type_error(&runtime, &mut ctx, "COMPLEX", &[Word::TRUE, imaginary]);
    assert_type_error(&runtime, &mut ctx, "PHASE", &[Word::TRUE]);
    assert_type_error(&runtime, &mut ctx, "CIS", &[Word::TRUE]);
}

#[test]
fn registration_matrix_asserts_numeric_constants_and_special_random_state() {
    let (runtime, mut ctx) = setup();
    let pi = common_lisp_symbol(&runtime, &mut ctx, "PI");
    assert_eq!(
        float(&ctx, symbol_value(&ctx, pi).unwrap()),
        std::f64::consts::PI
    );
    for name in [
        "SHORT-FLOAT-EPSILON",
        "SINGLE-FLOAT-EPSILON",
        "DOUBLE-FLOAT-EPSILON",
        "LONG-FLOAT-EPSILON",
    ] {
        let symbol = common_lisp_symbol(&runtime, &mut ctx, name);
        assert_eq!(
            float(&ctx, symbol_value(&ctx, symbol).unwrap()),
            f64::EPSILON
        );
    }
    for name in [
        "SHORT-FLOAT-NEGATIVE-EPSILON",
        "SINGLE-FLOAT-NEGATIVE-EPSILON",
        "DOUBLE-FLOAT-NEGATIVE-EPSILON",
        "LONG-FLOAT-NEGATIVE-EPSILON",
    ] {
        let symbol = common_lisp_symbol(&runtime, &mut ctx, name);
        assert_eq!(
            float(&ctx, symbol_value(&ctx, symbol).unwrap()),
            f64::EPSILON / 2.0
        );
    }
    for name in [
        "LEAST-POSITIVE-SHORT-FLOAT",
        "LEAST-POSITIVE-SINGLE-FLOAT",
        "LEAST-POSITIVE-DOUBLE-FLOAT",
        "LEAST-POSITIVE-LONG-FLOAT",
        "LEAST-POSITIVE-NORMALIZED-SHORT-FLOAT",
        "LEAST-POSITIVE-NORMALIZED-SINGLE-FLOAT",
        "LEAST-POSITIVE-NORMALIZED-DOUBLE-FLOAT",
        "LEAST-POSITIVE-NORMALIZED-LONG-FLOAT",
    ] {
        let symbol = common_lisp_symbol(&runtime, &mut ctx, name);
        assert_eq!(
            float(&ctx, symbol_value(&ctx, symbol).unwrap()),
            f64::MIN_POSITIVE
        );
    }
    for name in [
        "MOST-POSITIVE-SHORT-FLOAT",
        "MOST-POSITIVE-SINGLE-FLOAT",
        "MOST-POSITIVE-DOUBLE-FLOAT",
        "MOST-POSITIVE-LONG-FLOAT",
    ] {
        let symbol = common_lisp_symbol(&runtime, &mut ctx, name);
        assert_eq!(float(&ctx, symbol_value(&ctx, symbol).unwrap()), f64::MAX);
    }
    for name in [
        "MOST-NEGATIVE-SHORT-FLOAT",
        "MOST-NEGATIVE-SINGLE-FLOAT",
        "MOST-NEGATIVE-DOUBLE-FLOAT",
        "MOST-NEGATIVE-LONG-FLOAT",
    ] {
        let symbol = common_lisp_symbol(&runtime, &mut ctx, name);
        assert_eq!(float(&ctx, symbol_value(&ctx, symbol).unwrap()), -f64::MAX);
    }
    let positive_fixnum = common_lisp_symbol(&runtime, &mut ctx, "MOST-POSITIVE-FIXNUM");
    let negative_fixnum = common_lisp_symbol(&runtime, &mut ctx, "MOST-NEGATIVE-FIXNUM");
    assert_eq!(
        integer(&ctx, symbol_value(&ctx, positive_fixnum).unwrap()),
        i64::MAX as i128 >> ncl_sys::FIXNUM_TAG_BITS
    );
    assert_eq!(
        integer(&ctx, symbol_value(&ctx, negative_fixnum).unwrap()),
        i64::MIN as i128 >> ncl_sys::FIXNUM_TAG_BITS
    );
    let random_state = common_lisp_symbol(&runtime, &mut ctx, "*RANDOM-STATE*");
    let state = symbol_value(&ctx, random_state).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[state]),
        Ok(Word::TRUE)
    );
}

#[test]
fn random_state_shape_errors_return_concrete_type_errors() {
    let (runtime, mut ctx) = setup();
    let instance = make_instance(&mut ctx, &runtime, Word::fixnum(99), &[])
        .unwrap()
        .into();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[instance]),
        Ok(Word::NIL)
    );
    assert_type_error(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[instance]);
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    assert_type_error(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(4), instance]);
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[state]),
        Ok(Word::TRUE)
    );
}

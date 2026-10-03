#![allow(
    clippy::float_cmp,
    clippy::excessive_precision,
    clippy::map_unwrap_or,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert concrete numeric builtin behavior"
)]

use ncl_object::{
    Bignum, DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    bignum_limbs, bignum_sign, classify_object, complex_imag, complex_real, double_value,
    make_bignum_from_i128, make_complex, make_double, make_instance, make_ratio, ratio_denominator,
    ratio_numerator,
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

fn complex(ctx: &mut ThreadContext, runtime: &Runtime, real: f64, imag: f64) -> Word {
    let real = make_double(ctx, runtime, real).unwrap().into();
    let imag = make_double(ctx, runtime, imag).unwrap().into();
    make_complex(ctx, runtime, real, imag).unwrap().into()
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

fn assert_type_error(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) {
    assert_eq!(
        call(runtime, ctx, name, args),
        Err(ObjectError::TypeError),
        "{name}"
    );
}

fn assert_ratio_value(ctx: &ThreadContext, value: Word, numerator: i128, denominator: i128) {
    let ObjectRef::Ratio(value) = classify_object(ctx, value) else {
        panic!("expected ratio, got {:?}", classify_object(ctx, value));
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

#[test]
fn arithmetic_basic_exercises_empty_unary_mixed_and_overflow_paths() {
    let (runtime, mut ctx) = setup();
    assert_integer(&runtime, &mut ctx, "+", &[], 0);
    assert_integer(&runtime, &mut ctx, "*", &[], 1);
    assert_integer(&runtime, &mut ctx, "-", &[Word::fixnum(9)], -9);
    let reciprocal = call(&runtime, &mut ctx, "/", &[Word::fixnum(8)]).unwrap();
    assert_ratio_value(&ctx, reciprocal, 1, 8);
    let result = call(&runtime, &mut ctx, "/", &[Word::fixnum(1), Word::fixnum(8)]).unwrap();
    assert_ratio_value(&ctx, result, 1, 8);
    assert_integer(
        &runtime,
        &mut ctx,
        "+",
        &[Word::fixnum(4), Word::fixnum(-7), Word::fixnum(12)],
        9,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "-",
        &[Word::fixnum(4), Word::fixnum(-7), Word::fixnum(12)],
        -1,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "*",
        &[Word::fixnum(-2), Word::fixnum(3), Word::fixnum(-4)],
        24,
    );
    assert_integer(&runtime, &mut ctx, "1+", &[Word::fixnum(-1)], 0);
    assert_integer(&runtime, &mut ctx, "1-", &[Word::fixnum(0)], -1);

    let r1 = ratio(&mut ctx, &runtime, 2, 3);
    let r2 = ratio(&mut ctx, &runtime, -5, 7);
    let result = call(&runtime, &mut ctx, "+", &[r1, r2]).unwrap();
    assert_ratio_value(&ctx, result, -1, 21);
    let result = call(&runtime, &mut ctx, "-", &[r1, r2]).unwrap();
    assert_ratio_value(&ctx, result, 29, 21);
    let result = call(&runtime, &mut ctx, "*", &[r1, r2]).unwrap();
    assert_ratio_value(&ctx, result, -10, 21);
    let result = call(&runtime, &mut ctx, "/", &[r1, r2]).unwrap();
    assert_ratio_value(&ctx, result, -14, 15);

    let value = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    assert_float(&runtime, &mut ctx, "+", &[r1, value], 3.1666666666666665);
    assert_float(&runtime, &mut ctx, "*", &[value, r2], -1.7857142857142858);
    let z1 = complex(&mut ctx, &runtime, 2.0, 3.0);
    let z2 = complex(&mut ctx, &runtime, -1.0, 4.0);
    let result = call(&runtime, &mut ctx, "+", &[z1, z2]).unwrap();
    assert_eq!(pair(&ctx, result), (1.0, 7.0));
    let result = call(&runtime, &mut ctx, "-", &[z1, z2]).unwrap();
    assert_eq!(pair(&ctx, result), (3.0, -1.0));
    let result = call(&runtime, &mut ctx, "*", &[z1, z2]).unwrap();
    assert_eq!(pair(&ctx, result), (-14.0, 5.0));
    let result = call(&runtime, &mut ctx, "/", &[z1, z2]).unwrap();
    assert_eq!(pair(&ctx, result), (10.0 / 17.0, -11.0 / 17.0));
    let result = call(&runtime, &mut ctx, "+", &[z1, r1]).unwrap();
    assert_eq!(pair(&ctx, result), (2.6666666666666665, 3.0));
    let result = call(&runtime, &mut ctx, "-", &[r1, z1]).unwrap();
    assert_eq!(pair(&ctx, result), (-1.3333333333333335, -3.0));

    let max = bignum(&mut ctx, &runtime, i128::MAX);
    assert_type_error(&runtime, &mut ctx, "+", &[max, Word::fixnum(1)]);
    assert_type_error(&runtime, &mut ctx, "*", &[max, Word::fixnum(2)]);
    assert_type_error(&runtime, &mut ctx, "/", &[Word::fixnum(0)]);
    assert_type_error(&runtime, &mut ctx, "/", &[Word::fixnum(4), Word::fixnum(0)]);
    let minimum = bignum(&mut ctx, &runtime, i128::MIN);
    assert_type_error(&runtime, &mut ctx, "ABS", &[minimum]);
}

#[test]
fn arithmetic_core_predicates_cover_all_number_kinds_and_integer_boundaries() {
    let (runtime, mut ctx) = setup();
    let integer_value = Word::fixnum(-12);
    let ratio_value = ratio(&mut ctx, &runtime, 7, 5);
    let float_value = make_double(&mut ctx, &runtime, -2.25).unwrap().into();
    let complex_value = complex(&mut ctx, &runtime, 3.0, -4.0);
    for value in [integer_value, ratio_value, float_value, complex_value] {
        assert_boolean(&runtime, &mut ctx, "NUMBERP", &[value], true);
        assert_boolean(
            &runtime,
            &mut ctx,
            "REALP",
            &[value],
            value != complex_value,
        );
    }
    assert_boolean(&runtime, &mut ctx, "INTEGERP", &[integer_value], true);
    assert_boolean(&runtime, &mut ctx, "INTEGERP", &[ratio_value], false);
    assert_boolean(&runtime, &mut ctx, "INTEGERP", &[float_value], false);
    assert_boolean(&runtime, &mut ctx, "RATIONALP", &[integer_value], true);
    assert_boolean(&runtime, &mut ctx, "RATIONALP", &[ratio_value], true);
    assert_boolean(&runtime, &mut ctx, "RATIONALP", &[float_value], false);
    assert_boolean(&runtime, &mut ctx, "FLOATP", &[float_value], true);
    assert_boolean(&runtime, &mut ctx, "FLOATP", &[ratio_value], false);
    assert_boolean(&runtime, &mut ctx, "COMPLEXP", &[complex_value], true);
    assert_boolean(&runtime, &mut ctx, "COMPLEXP", &[float_value], false);
    assert_boolean(&runtime, &mut ctx, "NUMBERP", &[Word::NIL], false);
    assert_boolean(&runtime, &mut ctx, "REALP", &[Word::NIL], false);
    assert_boolean(&runtime, &mut ctx, "COMPLEXP", &[Word::NIL], false);

    assert_boolean(&runtime, &mut ctx, "ZEROP", &[Word::fixnum(0)], true);
    let zero_ratio = ratio(&mut ctx, &runtime, 0, 9);
    assert_boolean(&runtime, &mut ctx, "ZEROP", &[zero_ratio], true);
    assert_boolean(&runtime, &mut ctx, "ZEROP", &[float_value], false);
    assert_boolean(&runtime, &mut ctx, "PLUSP", &[Word::fixnum(12)], true);
    assert_boolean(&runtime, &mut ctx, "PLUSP", &[Word::fixnum(-12)], false);
    assert_boolean(&runtime, &mut ctx, "MINUSP", &[Word::fixnum(-12)], true);
    assert_boolean(&runtime, &mut ctx, "MINUSP", &[Word::fixnum(12)], false);
    assert_type_error(&runtime, &mut ctx, "PLUSP", &[complex_value]);
    assert_type_error(&runtime, &mut ctx, "MINUSP", &[complex_value]);
    assert_boolean(&runtime, &mut ctx, "EVENP", &[Word::fixnum(-12)], true);
    assert_boolean(&runtime, &mut ctx, "ODDP", &[Word::fixnum(-13)], true);
    let even_bignum = bignum(&mut ctx, &runtime, 1_i128 << 70);
    assert_boolean(&runtime, &mut ctx, "EVENP", &[even_bignum], true);
    let odd_bignum = bignum(&mut ctx, &runtime, (1_i128 << 70) + 1);
    assert_boolean(&runtime, &mut ctx, "ODDP", &[odd_bignum], true);
    assert_type_error(&runtime, &mut ctx, "EVENP", &[ratio_value]);
    assert_type_error(&runtime, &mut ctx, "ODDP", &[float_value]);
    assert_type_error(&runtime, &mut ctx, "NUMBERP", &[]);
    assert_type_error(&runtime, &mut ctx, "INTEGERP", &[]);
}

#[test]
fn bit_logic_and_fields_cover_empty_folds_signed_bits_and_layout_errors() {
    let (runtime, mut ctx) = setup();
    assert_integer(&runtime, &mut ctx, "LOGAND", &[], -1);
    assert_integer(&runtime, &mut ctx, "LOGIOR", &[], 0);
    assert_integer(&runtime, &mut ctx, "LOGXOR", &[], 0);
    assert_integer(&runtime, &mut ctx, "LOGEQV", &[], -1);
    let a = Word::fixnum(10);
    let b = Word::fixnum(12);
    assert_integer(&runtime, &mut ctx, "LOGAND", &[a, b], 8);
    assert_integer(&runtime, &mut ctx, "LOGIOR", &[a, b], 14);
    assert_integer(&runtime, &mut ctx, "LOGXOR", &[a, b], 6);
    assert_integer(&runtime, &mut ctx, "LOGNOT", &[a], -11);
    assert_integer(&runtime, &mut ctx, "LOGEQV", &[a, b], -7);
    assert_integer(&runtime, &mut ctx, "LOGNAND", &[a, b], -9);
    assert_integer(&runtime, &mut ctx, "LOGNOR", &[a, b], -15);
    assert_integer(&runtime, &mut ctx, "LOGANDC1", &[a, b], 4);
    assert_integer(&runtime, &mut ctx, "LOGANDC2", &[a, b], 2);
    assert_integer(&runtime, &mut ctx, "LOGORC1", &[a, b], -3);
    assert_integer(&runtime, &mut ctx, "LOGORC2", &[a, b], -5);
    assert_boolean(&runtime, &mut ctx, "LOGTEST", &[a, b], true);
    assert_boolean(
        &runtime,
        &mut ctx,
        "LOGTEST",
        &[Word::fixnum(2), Word::fixnum(4)],
        false,
    );
    assert_boolean(&runtime, &mut ctx, "LOGBITP", &[Word::fixnum(3), a], true);
    assert_boolean(&runtime, &mut ctx, "LOGBITP", &[Word::fixnum(4), a], false);
    assert_boolean(
        &runtime,
        &mut ctx,
        "LOGBITP",
        &[Word::fixnum(127), Word::fixnum(-1)],
        true,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "LOGBITP",
        &[Word::fixnum(127), Word::fixnum(1)],
        false,
    );
    assert_integer(&runtime, &mut ctx, "LOGCOUNT", &[Word::fixnum(10)], 2);
    assert_integer(&runtime, &mut ctx, "LOGCOUNT", &[Word::fixnum(-1)], 0);
    assert_integer(&runtime, &mut ctx, "INTEGER-LENGTH", &[Word::fixnum(10)], 4);
    assert_integer(&runtime, &mut ctx, "INTEGER-LENGTH", &[Word::fixnum(-1)], 0);
    assert_integer(
        &runtime,
        &mut ctx,
        "ASH",
        &[Word::fixnum(3), Word::fixnum(4)],
        48,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "ASH",
        &[Word::fixnum(-33), Word::fixnum(-2)],
        -9,
    );
    assert_type_error(&runtime, &mut ctx, "LOGNOT", &[]);
    assert_type_error(&runtime, &mut ctx, "LOGNAND", &[a]);
    assert_type_error(&runtime, &mut ctx, "LOGBITP", &[Word::fixnum(-1), a]);
    assert_type_error(&runtime, &mut ctx, "ASH", &[a]);

    let spec = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(5), Word::fixnum(7)],
    )
    .unwrap();
    assert_integer(&runtime, &mut ctx, "BYTE-SIZE", &[spec], 5);
    assert_integer(&runtime, &mut ctx, "BYTE-POSITION", &[spec], 7);
    assert_integer(
        &runtime,
        &mut ctx,
        "LDB",
        &[spec, Word::fixnum(0b1011_1000_0000)],
        23,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "LDB-TEST",
        &[spec, Word::fixnum(0b1_0000_0000)],
        true,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "LDB-TEST",
        &[spec, Word::fixnum(0b1)],
        false,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "DPB",
        &[Word::fixnum(3), spec, Word::fixnum(0)],
        3_i128 << 7,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "MASK-FIELD",
        &[spec, Word::fixnum(-1)],
        31_i128 << 7,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "DEPOSIT-FIELD",
        &[Word::fixnum(3), spec, Word::fixnum(-1)],
        -3969,
    );
    assert_type_error(&runtime, &mut ctx, "BYTE", &[Word::fixnum(5)]);
    assert_type_error(&runtime, &mut ctx, "BYTE-SIZE", &[Word::TRUE]);
    let wide_spec = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(128), Word::fixnum(0)],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "LDB", &[wide_spec, Word::fixnum(1)]),
        Err(ObjectError::Layout)
    );
    let far_spec = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(4), Word::fixnum(128)],
    )
    .unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "LDB-TEST", &[far_spec, Word::fixnum(1)]),
        Err(ObjectError::Layout)
    );
}

#[test]
fn random_branches_cover_default_clone_float_bignum_and_invalid_limits() {
    let (runtime, mut ctx) = setup();
    assert_integer(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(10)], 2);
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[state]),
        Ok(Word::TRUE)
    );
    assert_integer(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(10), state], 2);
    assert_integer(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(10), state], 3);
    let cloned = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[state]).unwrap();
    assert_integer(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(10), state], 4);
    assert_integer(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(10), cloned], 4);
    let float_limit = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    let float_state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    let float_result = call(&runtime, &mut ctx, "RANDOM", &[float_limit, float_state]).unwrap();
    assert_eq!(float(&ctx, float_result), 1.8091669593952568);
    let wide_state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]).unwrap();
    let wide_limit = bignum(&mut ctx, &runtime, 1_i128 << 100);
    let wide_result = call(&runtime, &mut ctx, "RANDOM", &[wide_limit, wide_state]).unwrap();
    assert_eq!(
        integer(&ctx, wide_result),
        180_583_166_191_023_279_029_129_237_250
    );
    let true_state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::TRUE]).unwrap();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[Word::TRUE]),
        Ok(Word::NIL)
    );
    assert_type_error(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::fixnum(7)]);
    assert_type_error(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(0), true_state]);
    assert_type_error(
        &runtime,
        &mut ctx,
        "RANDOM",
        &[Word::fixnum(-1), true_state],
    );
    assert_type_error(&runtime, &mut ctx, "RANDOM", &[Word::TRUE, true_state]);
    let infinity = make_double(&mut ctx, &runtime, f64::INFINITY)
        .unwrap()
        .into();
    let nan = make_double(&mut ctx, &runtime, f64::NAN).unwrap().into();
    assert_type_error(&runtime, &mut ctx, "RANDOM", &[infinity, true_state]);
    assert_type_error(&runtime, &mut ctx, "RANDOM", &[nan, true_state]);
    let invalid_state = make_instance(&mut ctx, &runtime, Word::fixnum(99), &[])
        .unwrap()
        .into();
    assert_eq!(
        call(&runtime, &mut ctx, "RANDOM-STATE-P", &[invalid_state]),
        Ok(Word::NIL)
    );
    assert_type_error(
        &runtime,
        &mut ctx,
        "RANDOM",
        &[Word::fixnum(3), invalid_state],
    );
}

#[test]
fn rational_float_branches_cover_exact_subnormal_nonfinite_and_optional_contracts() {
    let (runtime, mut ctx) = setup();
    let integer_value = bignum(&mut ctx, &runtime, 1_i128 << 70);
    assert_float(
        &runtime,
        &mut ctx,
        "FLOAT",
        &[integer_value],
        1_f64 * 2_f64.powi(70),
    );
    let ratio_value = ratio(&mut ctx, &runtime, -9, 4);
    assert_float(&runtime, &mut ctx, "FLOAT", &[ratio_value], -2.25);
    let format = make_double(&mut ctx, &runtime, 1.0).unwrap().into();
    assert_float(
        &runtime,
        &mut ctx,
        "FLOAT",
        &[Word::fixnum(-7), format],
        -7.0,
    );
    assert_type_error(&runtime, &mut ctx, "FLOAT", &[Word::fixnum(7), Word::TRUE]);
    let negative_denominator = ratio(&mut ctx, &runtime, -6, -4);
    let rationalized_negative =
        call(&runtime, &mut ctx, "RATIONAL", &[negative_denominator]).unwrap();
    let normalized_ratio = ratio(&mut ctx, &runtime, 3, 2);
    let rationalized_positive = call(&runtime, &mut ctx, "RATIONAL", &[normalized_ratio]).unwrap();
    assert_ratio_value(&ctx, rationalized_negative, 3, 2);
    assert_ratio_value(&ctx, rationalized_positive, 3, 2);
    let half = make_double(&mut ctx, &runtime, 0.5).unwrap().into();
    let rational_half = call(&runtime, &mut ctx, "RATIONAL", &[half]).unwrap();
    assert_ratio_value(&ctx, rational_half, 1, 2);
    let infinity = make_double(&mut ctx, &runtime, f64::INFINITY)
        .unwrap()
        .into();
    let nan = make_double(&mut ctx, &runtime, f64::NAN).unwrap().into();
    assert_type_error(&runtime, &mut ctx, "RATIONAL", &[infinity]);
    assert_type_error(&runtime, &mut ctx, "RATIONAL", &[nan]);
    let smallest = make_double(&mut ctx, &runtime, f64::from_bits(1))
        .unwrap()
        .into();
    let decoded = call(&runtime, &mut ctx, "DECODE-FLOAT", &[smallest]).unwrap();
    assert_eq!(float(&ctx, decoded), 2_f64.powi(-52));
    assert_eq!(integer(&ctx, ctx.values()[1]), -1022);
    assert_eq!(float(&ctx, ctx.values()[2]), 1.0);
    let integer_decoded = call(&runtime, &mut ctx, "INTEGER-DECODE-FLOAT", &[smallest]).unwrap();
    assert_eq!(integer(&ctx, integer_decoded), 1);
    assert_eq!(integer(&ctx, ctx.values()[1]), -1074);
    assert_eq!(integer(&ctx, ctx.values()[2]), 1);
    let normal = make_double(&mut ctx, &runtime, -8.0).unwrap().into();
    let decoded_normal = call(&runtime, &mut ctx, "INTEGER-DECODE-FLOAT", &[normal]).unwrap();
    assert_eq!(integer(&ctx, decoded_normal), 4_503_599_627_370_496);
    assert_eq!(integer(&ctx, ctx.values()[1]), -49);
    assert_eq!(integer(&ctx, ctx.values()[2]), -1);
    assert_float(
        &runtime,
        &mut ctx,
        "SCALE-FLOAT",
        &[normal, Word::fixnum(-3)],
        -1.0,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "SCALE-FLOAT",
        &[normal, Word::fixnum(3)],
        -64.0,
    );
    assert_float(&runtime, &mut ctx, "FLOAT-SIGN", &[normal], -1.0);
    assert_integer(&runtime, &mut ctx, "FLOAT-PRECISION", &[smallest], 1);
    assert_integer(&runtime, &mut ctx, "FLOAT-PRECISION", &[normal], 53);
    assert_integer(&runtime, &mut ctx, "FLOAT-DIGITS", &[normal], 53);
    assert_integer(&runtime, &mut ctx, "FLOAT-RADIX", &[normal], 2);
    let tolerance = make_double(&mut ctx, &runtime, 0.01).unwrap().into();
    let negative_float = make_double(&mut ctx, &runtime, -0.375).unwrap().into();
    let rationalized_float = call(
        &runtime,
        &mut ctx,
        "RATIONALIZE",
        &[negative_float, tolerance],
    )
    .unwrap();
    assert_ratio_value(&ctx, rationalized_float, -3, 8);
    let half = make_double(&mut ctx, &runtime, 0.5).unwrap().into();
    assert_type_error(&runtime, &mut ctx, "RATIONALIZE", &[half, Word::fixnum(0)]);
    let negative_tolerance = make_double(&mut ctx, &runtime, -0.1).unwrap().into();
    assert_type_error(
        &runtime,
        &mut ctx,
        "RATIONALIZE",
        &[half, negative_tolerance],
    );
    assert_type_error(&runtime, &mut ctx, "NUMERATOR", &[normal]);
}

#[test]
fn remainder_branches_cover_exact_sign_rules_float_fallbacks_and_root_edges() {
    let (runtime, mut ctx) = setup();
    assert_integer(
        &runtime,
        &mut ctx,
        "MOD",
        &[Word::fixnum(-17), Word::fixnum(5)],
        3,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "MOD",
        &[Word::fixnum(17), Word::fixnum(-5)],
        -3,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "REM",
        &[Word::fixnum(-17), Word::fixnum(5)],
        -2,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "REM",
        &[Word::fixnum(17), Word::fixnum(-5)],
        2,
    );
    let value = ratio(&mut ctx, &runtime, 7, 3);
    let divisor = ratio(&mut ctx, &runtime, 5, 2);
    let result = call(&runtime, &mut ctx, "MOD", &[value, divisor]).unwrap();
    assert_ratio_value(&ctx, result, 7, 3);
    let result = call(&runtime, &mut ctx, "REM", &[value, divisor]).unwrap();
    assert_ratio_value(&ctx, result, 7, 3);
    let float_value = make_double(&mut ctx, &runtime, -7.5).unwrap().into();
    let float_divisor = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    assert_float(
        &runtime,
        &mut ctx,
        "MOD",
        &[float_value, float_divisor],
        0.5,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "REM",
        &[float_value, float_divisor],
        -1.5,
    );
    assert_integer(&runtime, &mut ctx, "GCD", &[], 0);
    assert_integer(
        &runtime,
        &mut ctx,
        "GCD",
        &[Word::fixnum(-48), Word::fixnum(18)],
        6,
    );
    let wide_integer = bignum(&mut ctx, &runtime, 1_i128 << 70);
    assert_integer(
        &runtime,
        &mut ctx,
        "GCD",
        &[wide_integer, Word::fixnum(0)],
        1_i128 << 70,
    );
    assert_integer(&runtime, &mut ctx, "LCM", &[], 1);
    assert_integer(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(-12), Word::fixnum(18)],
        36,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(0), Word::fixnum(18)],
        0,
    );
    assert_integer(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(0)], 0);
    assert_integer(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(1)], 1);
    assert_integer(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(99)], 9);
    let square = bignum(&mut ctx, &runtime, 1_i128 << 100);
    assert_integer(&runtime, &mut ctx, "ISQRT", &[square], 1_i128 << 50);
    assert_type_error(
        &runtime,
        &mut ctx,
        "MOD",
        &[Word::fixnum(4), Word::fixnum(0)],
    );
    assert_type_error(
        &runtime,
        &mut ctx,
        "REM",
        &[Word::fixnum(4), Word::fixnum(0)],
    );
    assert_type_error(&runtime, &mut ctx, "MOD", &[Word::TRUE, Word::fixnum(2)]);
    assert_type_error(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(-1)]);
    assert_type_error(&runtime, &mut ctx, "GCD", &[Word::TRUE]);
    assert_type_error(&runtime, &mut ctx, "LCM", &[Word::TRUE]);
}

#[test]
fn rounding_branches_cover_all_modes_ratio_signs_float_fallbacks_and_ties() {
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
        assert_eq!(integer(&ctx, result), quotient, "{name}");
        assert_eq!(
            integer(&ctx, ctx.values()[1]),
            remainder,
            "{name} remainder"
        );
    }
    let ratio_value = ratio(&mut ctx, &runtime, -7, 3);
    let ratio_divisor = ratio(&mut ctx, &runtime, 5, 2);
    let result = call(&runtime, &mut ctx, "FLOOR", &[ratio_value, ratio_divisor]).unwrap();
    assert_eq!(integer(&ctx, result), -1);
    assert_ratio_value(&ctx, ctx.values()[1], 1, 6);
    let result = call(&runtime, &mut ctx, "CEILING", &[ratio_value, ratio_divisor]).unwrap();
    assert_eq!(integer(&ctx, result), 0);
    assert_ratio_value(&ctx, ctx.values()[1], -7, 3);
    let result = call(
        &runtime,
        &mut ctx,
        "TRUNCATE",
        &[ratio_value, ratio_divisor],
    )
    .unwrap();
    assert_eq!(integer(&ctx, result), 0);
    assert_ratio_value(&ctx, ctx.values()[1], -7, 3);
    let result = call(
        &runtime,
        &mut ctx,
        "ROUND",
        &[Word::fixnum(7), Word::fixnum(2)],
    )
    .unwrap();
    assert_eq!(integer(&ctx, result), 4);
    assert_eq!(integer(&ctx, ctx.values()[1]), -1);
    let float_value = make_double(&mut ctx, &runtime, -7.5).unwrap().into();
    let float_divisor = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    assert_float(
        &runtime,
        &mut ctx,
        "FFLOOR",
        &[float_value, float_divisor],
        -4.0,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "FCEILING",
        &[float_value, float_divisor],
        -3.0,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "FTRUNCATE",
        &[float_value, float_divisor],
        -3.0,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "FROUND",
        &[float_value, float_divisor],
        -4.0,
    );
    assert_eq!(float(&ctx, ctx.values()[1]), 0.5);
    let tie_even = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    let tie_odd = make_double(&mut ctx, &runtime, 3.5).unwrap().into();
    assert_float(&runtime, &mut ctx, "FROUND", &[tie_even], 2.0);
    assert_float(&runtime, &mut ctx, "FROUND", &[tie_odd], 4.0);
    assert_type_error(
        &runtime,
        &mut ctx,
        "FLOOR",
        &[Word::fixnum(1), Word::fixnum(0)],
    );
    assert_type_error(&runtime, &mut ctx, "ROUND", &[Word::TRUE]);
    assert_type_error(
        &runtime,
        &mut ctx,
        "FCEILING",
        &[Word::fixnum(1), Word::TRUE],
    );
}

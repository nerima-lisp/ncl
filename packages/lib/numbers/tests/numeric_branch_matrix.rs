#![allow(
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert exact numeric builtin behavior"
)]

use ncl_object::{
    Bignum, DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    bignum_limbs, bignum_sign, classify_object, complex_imag, complex_real, double_value,
    make_bignum_from_i128, make_complex, make_double, make_ratio, ratio_denominator,
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
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    numerator: i128,
    denominator: i128,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    let ObjectRef::Ratio(result) = classify_object(ctx, result) else {
        panic!(
            "{name}: expected ratio, got {:?}",
            classify_object(ctx, result)
        );
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

#[test]
fn subtraction_division_and_magnitude_cover_exact_and_complex_kinds() {
    let (runtime, mut ctx) = setup();
    let one_seventh = ratio(&mut ctx, &runtime, 1, 7);
    let three_fifths = ratio(&mut ctx, &runtime, 3, 5);
    let float_value = make_double(&mut ctx, &runtime, -6.25).unwrap().into();
    let z = complex(&mut ctx, &runtime, 5.0, -12.0);
    let zero_complex = complex(&mut ctx, &runtime, 0.0, 0.0);

    assert_integer(&runtime, &mut ctx, "-", &[Word::fixnum(37)], -37);
    assert_ratio(&runtime, &mut ctx, "-", &[one_seventh], -1, 7);
    assert_float(&runtime, &mut ctx, "-", &[float_value], 6.25);
    let negative_complex = call(&runtime, &mut ctx, "-", &[z]).unwrap();
    assert_eq!(
        float(
            &ctx,
            complex_real(
                &ctx,
                ncl_object::Complex::from_word(match classify_object(&ctx, negative_complex) {
                    ObjectRef::Complex(value) => value,
                    other => panic!("expected complex, got {other:?}"),
                },)
            )
            .unwrap()
        ),
        -5.0
    );

    assert_ratio(&runtime, &mut ctx, "/", &[Word::fixnum(3)], 1, 3);
    assert_ratio(&runtime, &mut ctx, "/", &[one_seventh, three_fifths], 5, 21);
    assert_ratio(
        &runtime,
        &mut ctx,
        "/",
        &[three_fifths, Word::fixnum(2)],
        3,
        10,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "/",
        &[Word::fixnum(2), three_fifths],
        10,
        3,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "/",
        &[float_value, Word::fixnum(2)],
        -3.125,
    );
    let divisor_complex = complex(&mut ctx, &runtime, 1.0, 2.0);
    let complex_quotient = call(&runtime, &mut ctx, "/", &[z, divisor_complex]).unwrap();
    let ObjectRef::Complex(value) = classify_object(&ctx, complex_quotient) else {
        panic!("expected complex quotient");
    };
    let value = ncl_object::Complex::from_word(value);
    assert_eq!(float(&ctx, complex_real(&ctx, value).unwrap()), -3.8);
    assert_eq!(float(&ctx, complex_imag(&ctx, value).unwrap()), -4.4);

    let negative_ratio = ratio(&mut ctx, &runtime, -11, 13);
    assert_ratio(&runtime, &mut ctx, "ABS", &[negative_ratio], 11, 13);
    assert_float(&runtime, &mut ctx, "ABS", &[float_value], 6.25);
    assert_float(&runtime, &mut ctx, "ABS", &[z], 13.0);
    let near_minimum = bignum(&mut ctx, &runtime, i128::MIN + 1);
    assert_integer(
        &runtime,
        &mut ctx,
        "ABS",
        &[near_minimum],
        170_141_183_460_469_231_731_687_303_715_884_105_727,
    );

    let positive_ratio = ratio(&mut ctx, &runtime, 8, 21);
    assert_integer(&runtime, &mut ctx, "SIGNUM", &[positive_ratio], 1);
    assert_float(&runtime, &mut ctx, "SIGNUM", &[float_value], -1.0);
    let zero_sign = call(&runtime, &mut ctx, "SIGNUM", &[zero_complex]).unwrap();
    let ObjectRef::Complex(value) = classify_object(&ctx, zero_sign) else {
        panic!("expected zero complex sign");
    };
    let value = ncl_object::Complex::from_word(value);
    assert_eq!(float(&ctx, complex_real(&ctx, value).unwrap()), 0.0);
    assert_eq!(float(&ctx, complex_imag(&ctx, value).unwrap()), 0.0);

    assert_type_error(&runtime, &mut ctx, "-", &[]);
    assert_type_error(&runtime, &mut ctx, "/", &[]);
    let zero_float = make_double(&mut ctx, &runtime, -0.0).unwrap().into();
    for divisor in [
        Word::fixnum(0),
        ratio(&mut ctx, &runtime, 0, 9),
        zero_float,
        zero_complex,
    ] {
        assert_type_error(&runtime, &mut ctx, "/", &[Word::fixnum(9), divisor]);
    }
}

#[test]
fn arithmetic_comparisons_cover_mixed_order_eql_and_extrema() {
    let (runtime, mut ctx) = setup();
    let ratio_value = ratio(&mut ctx, &runtime, 7, 4);
    let same_ratio = ratio(&mut ctx, &runtime, 7, 4);
    let float_value = make_double(&mut ctx, &runtime, 1.75).unwrap().into();
    let larger_float = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    let bignum_value = bignum(&mut ctx, &runtime, 1_i128 << 70);
    let same_bignum = bignum(&mut ctx, &runtime, 1_i128 << 70);
    let negative_zero = make_double(&mut ctx, &runtime, -0.0).unwrap().into();
    let positive_zero = make_double(&mut ctx, &runtime, 0.0).unwrap().into();

    assert_boolean(&runtime, &mut ctx, "=", &[ratio_value, float_value], true);
    assert_boolean(&runtime, &mut ctx, "=", &[larger_float, ratio_value], false);
    assert_boolean(&runtime, &mut ctx, "/=", &[ratio_value, larger_float], true);
    assert_boolean(&runtime, &mut ctx, "/=", &[ratio_value, same_ratio], false);
    assert_boolean(&runtime, &mut ctx, "<", &[ratio_value, larger_float], true);
    assert_boolean(&runtime, &mut ctx, ">", &[larger_float, ratio_value], true);
    assert_boolean(&runtime, &mut ctx, "<=", &[ratio_value, float_value], true);
    assert_boolean(&runtime, &mut ctx, ">=", &[float_value, ratio_value], true);
    assert_boolean(&runtime, &mut ctx, "<", &[larger_float, ratio_value], false);
    assert_boolean(&runtime, &mut ctx, ">", &[ratio_value, larger_float], false);
    assert_boolean(
        &runtime,
        &mut ctx,
        "<=",
        &[larger_float, ratio_value],
        false,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        ">=",
        &[ratio_value, larger_float],
        false,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "EQL",
        &[same_bignum, bignum_value],
        true,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "EQL",
        &[negative_zero, positive_zero],
        false,
    );
    assert_boolean(&runtime, &mut ctx, "EQ", &[ratio_value, same_ratio], false);
    assert_boolean(
        &runtime,
        &mut ctx,
        "EQ",
        &[Word::fixnum(12), Word::fixnum(12)],
        true,
    );
    assert_type_error(&runtime, &mut ctx, "EQ", &[Word::fixnum(1)]);
    assert_type_error(&runtime, &mut ctx, "EQL", &[Word::fixnum(1)]);

    assert_integer(
        &runtime,
        &mut ctx,
        "MAX",
        &[Word::fixnum(-8), ratio_value, larger_float, bignum_value],
        1_i128 << 70,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "MIN",
        &[ratio_value, larger_float, Word::fixnum(3)],
        7,
        4,
    );
    assert_float(&runtime, &mut ctx, "MAX", &[float_value, ratio_value], 1.75);
    assert_float(
        &runtime,
        &mut ctx,
        "MIN",
        &[larger_float, float_value],
        1.75,
    );
    assert_integer(&runtime, &mut ctx, "MAX", &[Word::fixnum(-8)], -8);
    assert_ratio(&runtime, &mut ctx, "MIN", &[ratio_value], 7, 4);
    assert_type_error(&runtime, &mut ctx, "MAX", &[]);
    assert_type_error(&runtime, &mut ctx, "MIN", &[]);
}

#[test]
fn numeric_predicates_cover_false_branches_signs_and_invalid_kinds() {
    let (runtime, mut ctx) = setup();
    let integer_value = Word::fixnum(-9);
    let ratio_value = ratio(&mut ctx, &runtime, -5, 8);
    let float_value = make_double(&mut ctx, &runtime, 2.25).unwrap().into();
    let zero_ratio = ratio(&mut ctx, &runtime, 0, 17);
    let complex_value = complex(&mut ctx, &runtime, 3.0, -4.0);
    let zero_complex = complex(&mut ctx, &runtime, 0.0, 0.0);

    assert_boolean(&runtime, &mut ctx, "NUMBERP", &[integer_value], true);
    assert_boolean(&runtime, &mut ctx, "NUMBERP", &[complex_value], true);
    assert_boolean(&runtime, &mut ctx, "NUMBERP", &[Word::TRUE], false);
    assert_boolean(&runtime, &mut ctx, "INTEGERP", &[integer_value], true);
    assert_boolean(&runtime, &mut ctx, "INTEGERP", &[ratio_value], false);
    assert_boolean(&runtime, &mut ctx, "INTEGERP", &[float_value], false);
    assert_boolean(&runtime, &mut ctx, "INTEGERP", &[complex_value], false);
    assert_boolean(&runtime, &mut ctx, "RATIONALP", &[integer_value], true);
    assert_boolean(&runtime, &mut ctx, "RATIONALP", &[ratio_value], true);
    assert_boolean(&runtime, &mut ctx, "RATIONALP", &[float_value], false);
    assert_boolean(&runtime, &mut ctx, "RATIONALP", &[complex_value], false);
    assert_boolean(&runtime, &mut ctx, "FLOATP", &[float_value], true);
    assert_boolean(&runtime, &mut ctx, "FLOATP", &[integer_value], false);
    assert_boolean(&runtime, &mut ctx, "FLOATP", &[complex_value], false);
    assert_boolean(&runtime, &mut ctx, "REALP", &[integer_value], true);
    assert_boolean(&runtime, &mut ctx, "REALP", &[ratio_value], true);
    assert_boolean(&runtime, &mut ctx, "REALP", &[float_value], true);
    assert_boolean(&runtime, &mut ctx, "REALP", &[complex_value], false);
    assert_boolean(&runtime, &mut ctx, "COMPLEXP", &[complex_value], true);
    assert_boolean(&runtime, &mut ctx, "COMPLEXP", &[float_value], false);
    assert_boolean(&runtime, &mut ctx, "COMPLEXP", &[ratio_value], false);
    assert_boolean(&runtime, &mut ctx, "COMPLEXP", &[Word::TRUE], false);

    assert_boolean(&runtime, &mut ctx, "ZEROP", &[zero_ratio], true);
    assert_boolean(&runtime, &mut ctx, "ZEROP", &[Word::fixnum(0)], true);
    assert_boolean(&runtime, &mut ctx, "ZEROP", &[float_value], false);
    assert_boolean(&runtime, &mut ctx, "ZEROP", &[zero_complex], true);
    assert_boolean(&runtime, &mut ctx, "ZEROP", &[complex_value], false);
    assert_boolean(&runtime, &mut ctx, "PLUSP", &[Word::fixnum(0)], false);
    assert_boolean(&runtime, &mut ctx, "PLUSP", &[ratio_value], false);
    assert_boolean(&runtime, &mut ctx, "PLUSP", &[float_value], true);
    assert_boolean(&runtime, &mut ctx, "MINUSP", &[Word::fixnum(0)], false);
    assert_boolean(&runtime, &mut ctx, "MINUSP", &[ratio_value], true);
    assert_boolean(&runtime, &mut ctx, "MINUSP", &[float_value], false);
    assert_type_error(&runtime, &mut ctx, "PLUSP", &[complex_value]);
    assert_type_error(&runtime, &mut ctx, "MINUSP", &[complex_value]);

    for (name, value, expected) in [
        ("EVENP", Word::fixnum(-12), true),
        ("EVENP", Word::fixnum(-11), false),
        ("ODDP", Word::fixnum(-11), true),
        ("ODDP", Word::fixnum(-12), false),
    ] {
        assert_boolean(&runtime, &mut ctx, name, &[value], expected);
    }
    let wide_even = bignum(&mut ctx, &runtime, (1_i128 << 80) + 2);
    assert_boolean(&runtime, &mut ctx, "EVENP", &[wide_even], true);
    assert_type_error(&runtime, &mut ctx, "EVENP", &[ratio_value]);
    assert_type_error(&runtime, &mut ctx, "ODDP", &[float_value]);
    for name in [
        "NUMBERP",
        "INTEGERP",
        "RATIONALP",
        "FLOATP",
        "REALP",
        "COMPLEXP",
        "ZEROP",
        "PLUSP",
        "MINUSP",
        "EVENP",
        "ODDP",
    ] {
        assert_type_error(&runtime, &mut ctx, name, &[]);
    }
}

#[test]
fn bitwise_logic_covers_empty_folds_signed_bits_and_shift_errors() {
    let (runtime, mut ctx) = setup();
    let a = Word::fixnum(0b1011_0010);
    let b = Word::fixnum(0b0110_1100);
    let negative = Word::fixnum(-0b1_0101);
    let wide = bignum(&mut ctx, &runtime, 1_i128 << 70);

    assert_integer(&runtime, &mut ctx, "LOGAND", &[], -1);
    assert_integer(&runtime, &mut ctx, "LOGIOR", &[], 0);
    assert_integer(&runtime, &mut ctx, "LOGXOR", &[], 0);
    assert_integer(&runtime, &mut ctx, "LOGEQV", &[], -1);
    assert_integer(&runtime, &mut ctx, "LOGAND", &[a, b, negative], 32);
    assert_integer(&runtime, &mut ctx, "LOGIOR", &[a, b], 254);
    assert_integer(&runtime, &mut ctx, "LOGXOR", &[a, b], 222);
    assert_integer(&runtime, &mut ctx, "LOGEQV", &[a, b], -223);
    assert_integer(&runtime, &mut ctx, "LOGNOT", &[negative], 20);
    assert_integer(&runtime, &mut ctx, "LOGNAND", &[a, b], -33);
    assert_integer(&runtime, &mut ctx, "LOGNOR", &[a, b], -255);
    assert_integer(&runtime, &mut ctx, "LOGANDC1", &[a, b], 76);
    assert_integer(&runtime, &mut ctx, "LOGANDC2", &[a, b], 146);
    assert_integer(&runtime, &mut ctx, "LOGORC1", &[a, b], -147);
    assert_integer(&runtime, &mut ctx, "LOGORC2", &[a, b], -77);
    assert_integer(
        &runtime,
        &mut ctx,
        "LOGAND",
        &[wide, negative],
        1_i128 << 70,
    );
    assert_boolean(&runtime, &mut ctx, "LOGTEST", &[a, b], true);
    assert_boolean(
        &runtime,
        &mut ctx,
        "LOGTEST",
        &[Word::fixnum(2), Word::fixnum(4)],
        false,
    );
    assert_boolean(&runtime, &mut ctx, "LOGBITP", &[Word::fixnum(7), a], true);
    assert_boolean(&runtime, &mut ctx, "LOGBITP", &[Word::fixnum(8), a], false);
    assert_boolean(
        &runtime,
        &mut ctx,
        "LOGBITP",
        &[Word::fixnum(127), Word::fixnum(9)],
        false,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "LOGBITP",
        &[Word::fixnum(127), negative],
        true,
    );
    assert_integer(&runtime, &mut ctx, "LOGCOUNT", &[Word::fixnum(0)], 0);
    assert_integer(&runtime, &mut ctx, "LOGCOUNT", &[Word::fixnum(-1)], 0);
    assert_integer(&runtime, &mut ctx, "LOGCOUNT", &[wide], 1);
    assert_integer(&runtime, &mut ctx, "INTEGER-LENGTH", &[Word::fixnum(0)], 0);
    assert_integer(&runtime, &mut ctx, "INTEGER-LENGTH", &[Word::fixnum(1)], 1);
    assert_integer(&runtime, &mut ctx, "INTEGER-LENGTH", &[Word::fixnum(-1)], 0);
    assert_integer(&runtime, &mut ctx, "INTEGER-LENGTH", &[negative], 5);
    assert_integer(
        &runtime,
        &mut ctx,
        "ASH",
        &[Word::fixnum(-3), Word::fixnum(4)],
        -48,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "ASH",
        &[Word::fixnum(-33), Word::fixnum(-3)],
        -5,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "ASH",
        &[Word::fixnum(1), Word::fixnum(70)],
        1_i128 << 70,
    );
    assert_type_error(&runtime, &mut ctx, "LOGBITP", &[Word::fixnum(-1), a]);
    assert_type_error(&runtime, &mut ctx, "LOGNOT", &[]);
    assert_type_error(&runtime, &mut ctx, "LOGNAND", &[a]);
    assert_type_error(&runtime, &mut ctx, "ASH", &[a]);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "ASH",
            &[Word::fixnum(1), Word::fixnum(128)]
        ),
        Err(ObjectError::Layout)
    );
}

#[test]
fn byte_fields_cover_zero_full_width_negative_values_and_layout_errors() {
    let (runtime, mut ctx) = setup();
    let narrow = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(5), Word::fixnum(4)],
    )
    .unwrap();
    let empty = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(0), Word::fixnum(0)],
    )
    .unwrap();
    let full = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(127), Word::fixnum(0)],
    )
    .unwrap();

    assert_integer(&runtime, &mut ctx, "BYTE-SIZE", &[narrow], 5);
    assert_integer(&runtime, &mut ctx, "BYTE-POSITION", &[narrow], 4);
    assert_integer(&runtime, &mut ctx, "BYTE-SIZE", &[empty], 0);
    assert_integer(&runtime, &mut ctx, "BYTE-POSITION", &[empty], 0);
    assert_integer(
        &runtime,
        &mut ctx,
        "LDB",
        &[narrow, Word::fixnum(0b1_0110_1010)],
        22,
    );
    assert_integer(&runtime, &mut ctx, "LDB", &[empty, Word::fixnum(-1)], 0);
    assert_boolean(
        &runtime,
        &mut ctx,
        "LDB-TEST",
        &[narrow, Word::fixnum(0)],
        false,
    );
    assert_boolean(
        &runtime,
        &mut ctx,
        "LDB-TEST",
        &[narrow, Word::fixnum(0b1_0000)],
        true,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "LDB",
        &[full, Word::fixnum(-1)],
        i128::MAX,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "MASK-FIELD",
        &[narrow, Word::fixnum(-1)],
        0b1_1111_0000,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "MASK-FIELD",
        &[empty, Word::fixnum(-1)],
        0,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "DPB",
        &[Word::fixnum(0b1_1111), narrow, Word::fixnum(-1)],
        -1,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "DEPOSIT-FIELD",
        &[Word::fixnum(0b1_0000_0000), narrow, Word::fixnum(-1)],
        -241,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "DEPOSIT-FIELD",
        &[Word::fixnum(0), empty, Word::fixnum(-9)],
        -9,
    );
    let invalid_size = Word::fixnum(128);
    let invalid_position = Word::fixnum(128_i64 << 32);
    assert_integer(&runtime, &mut ctx, "BYTE-SIZE", &[invalid_size], 128);
    assert_integer(
        &runtime,
        &mut ctx,
        "BYTE-POSITION",
        &[invalid_position],
        128,
    );
    for name in ["LDB", "LDB-TEST", "MASK-FIELD", "DPB", "DEPOSIT-FIELD"] {
        let args = match name {
            "BYTE-SIZE" | "BYTE-POSITION" => vec![invalid_size],
            "LDB" | "LDB-TEST" | "MASK-FIELD" => vec![invalid_size, Word::fixnum(1)],
            _ => vec![Word::fixnum(1), invalid_position, Word::fixnum(0)],
        };
        assert_eq!(
            call(&runtime, &mut ctx, name, &args),
            Err(ObjectError::Layout),
            "{name}"
        );
    }
    assert_type_error(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(-1), Word::fixnum(0)],
    );
    assert_type_error(&runtime, &mut ctx, "BYTE-SIZE", &[]);
    assert_type_error(&runtime, &mut ctx, "LDB", &[narrow]);
}

#[test]
fn remainder_and_integer_helpers_cover_sign_cross_products_and_boundaries() {
    let (runtime, mut ctx) = setup();
    let seven_thirds = ratio(&mut ctx, &runtime, 7, 3);
    let negative_two_thirds = ratio(&mut ctx, &runtime, -2, 3);
    let negative_three_halves = ratio(&mut ctx, &runtime, -3, 2);
    let float_value = make_double(&mut ctx, &runtime, 7.75).unwrap().into();
    let float_divisor = make_double(&mut ctx, &runtime, 2.5).unwrap().into();

    assert_ratio(
        &runtime,
        &mut ctx,
        "MOD",
        &[seven_thirds, negative_three_halves],
        -2,
        3,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "REM",
        &[seven_thirds, negative_three_halves],
        5,
        6,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "MOD",
        &[negative_two_thirds, Word::fixnum(2)],
        4,
        3,
    );
    assert_ratio(
        &runtime,
        &mut ctx,
        "REM",
        &[negative_two_thirds, Word::fixnum(2)],
        -2,
        3,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "MOD",
        &[float_value, Word::fixnum(2)],
        1.75,
    );
    assert_float(
        &runtime,
        &mut ctx,
        "REM",
        &[float_value, float_divisor],
        0.25,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "GCD",
        &[Word::fixnum(-84), Word::fixnum(126), Word::fixnum(210)],
        42,
    );
    assert_integer(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(-9), Word::fixnum(12), Word::fixnum(25)],
        900,
    );
    assert_integer(&runtime, &mut ctx, "GCD", &[Word::fixnum(0)], 0);
    assert_integer(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(0), Word::fixnum(15)],
        0,
    );
    assert_integer(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(1)], 1);
    assert_integer(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(24)], 4);
    let large_square = bignum(&mut ctx, &runtime, (1_i128 << 100) + 9);
    assert_integer(&runtime, &mut ctx, "ISQRT", &[large_square], 1_i128 << 50);
    assert_type_error(&runtime, &mut ctx, "GCD", &[float_value]);
    assert_type_error(&runtime, &mut ctx, "LCM", &[negative_two_thirds]);
}

#[test]
fn rounding_matrix_covers_exact_ties_optional_divisors_and_float_modes() {
    let (runtime, mut ctx) = setup();
    for (name, value, divisor, quotient, remainder) in [
        ("FLOOR", 11, 4, 2, 3),
        ("CEILING", 11, 4, 3, -1),
        ("TRUNCATE", -11, 4, -2, -3),
        ("ROUND", 11, 4, 3, -1),
        ("ROUND", 10, 4, 2, 2),
        ("ROUND", 14, 4, 4, -2),
    ] {
        let result = call(
            &runtime,
            &mut ctx,
            name,
            &[Word::fixnum(value), Word::fixnum(divisor)],
        )
        .unwrap();
        assert_eq!(integer(&ctx, result), quotient, "{name} quotient");
        assert_eq!(
            integer(&ctx, ctx.values()[1]),
            remainder,
            "{name} remainder"
        );
    }

    let exact_ratio = ratio(&mut ctx, &runtime, 13, 4);
    let ratio_divisor = ratio(&mut ctx, &runtime, 3, 2);
    let result = call(&runtime, &mut ctx, "CEILING", &[exact_ratio, ratio_divisor]).unwrap();
    assert_eq!(integer(&ctx, result), 3);
    assert_ratio_value(&ctx, ctx.values()[1], -5, 4);

    let value = make_double(&mut ctx, &runtime, -11.5).unwrap().into();
    let divisor = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    for (name, quotient, remainder) in [
        ("FFLOOR", -6.0, 0.5),
        ("FCEILING", -5.0, -1.5),
        ("FTRUNCATE", -5.0, -1.5),
        ("FROUND", -6.0, 0.5),
    ] {
        let result = call(&runtime, &mut ctx, name, &[value, divisor]).unwrap();
        assert_eq!(float(&ctx, result), quotient, "{name} quotient");
        assert_eq!(float(&ctx, ctx.values()[1]), remainder, "{name} remainder");
    }
    let no_divisor = call(&runtime, &mut ctx, "FLOOR", &[Word::fixnum(-9)]).unwrap();
    assert_eq!(integer(&ctx, no_divisor), -9);
    assert_eq!(integer(&ctx, ctx.values()[1]), 0);
    let rounded_float = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    let rounded = call(&runtime, &mut ctx, "FROUND", &[rounded_float]).unwrap();
    assert_eq!(float(&ctx, rounded), 2.0);
    assert_eq!(float(&ctx, ctx.values()[1]), 0.5);
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
        assert_type_error(
            &runtime,
            &mut ctx,
            name,
            &[Word::fixnum(5), Word::fixnum(0)],
        );
    }
}

fn assert_ratio_value(ctx: &ThreadContext, value: Word, numerator: i128, denominator: i128) {
    let ObjectRef::Ratio(value) = classify_object(ctx, value) else {
        panic!("expected ratio remainder");
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
fn exact_power_paths_cover_repeated_squaring_and_ratio_inversion() {
    let (runtime, mut ctx) = setup();
    assert_integer(
        &runtime,
        &mut ctx,
        "EXPT",
        &[Word::fixnum(-3), Word::fixnum(5)],
        -243,
    );
    let negative_ratio = ratio(&mut ctx, &runtime, -2, 5);
    assert_ratio(
        &runtime,
        &mut ctx,
        "EXPT",
        &[negative_ratio, Word::fixnum(-3)],
        -125,
        8,
    );
    let large_base = bignum(&mut ctx, &runtime, 2_i128 << 40);
    assert_integer(
        &runtime,
        &mut ctx,
        "EXPT",
        &[large_base, Word::fixnum(2)],
        2_i128 << 81,
    );
    let overflowing_base = bignum(&mut ctx, &runtime, i128::MAX);
    let overflow = call(
        &runtime,
        &mut ctx,
        "EXPT",
        &[overflowing_base, Word::fixnum(2)],
    )
    .unwrap();
    let overflow_value = float(&ctx, overflow);
    let expected_overflow = 2.8948022309329049e76;
    assert!(
        ((overflow_value - expected_overflow) / expected_overflow).abs() < 1e-12,
        "EXPT overflow value: {overflow_value}"
    );
    let float_exponent = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    let fallback = call(
        &runtime,
        &mut ctx,
        "EXPT",
        &[Word::fixnum(9), float_exponent],
    )
    .unwrap();
    assert!((float(&ctx, fallback) - 81.0).abs() < 1e-12);
}

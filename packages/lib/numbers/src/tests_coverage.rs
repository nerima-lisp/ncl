#![allow(
    clippy::float_cmp,
    clippy::unreadable_literal,
    clippy::unwrap_used,
    missing_docs
)]

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectError, ObjectRef, Ratio, Runtime, ThreadContext, Word,
    classify_object, complex_imag, complex_real, double_value, make_bignum_from_i128, make_complex,
    make_double, make_ratio, ratio_denominator, ratio_numerator,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    super::register(&runtime).unwrap();
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

fn ratio_parts(ctx: &ThreadContext, value: Word) -> (i128, i128) {
    let ObjectRef::Ratio(value) = classify_object(ctx, value) else {
        panic!("expected ratio")
    };
    let value = Ratio::from_word(value);
    (
        integer(ctx, ratio_numerator(ctx, value).unwrap()),
        integer(ctx, ratio_denominator(ctx, value).unwrap()),
    )
}

fn complex_parts(ctx: &ThreadContext, value: Word) -> (f64, f64) {
    let ObjectRef::Complex(value) = classify_object(ctx, value) else {
        panic!("expected complex")
    };
    let value = ncl_object::Complex::from_word(value);
    (
        float(ctx, complex_real(ctx, value).unwrap()),
        float(ctx, complex_imag(ctx, value).unwrap()),
    )
}

fn call_integer(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> i128 {
    let result = call(runtime, ctx, name, args).unwrap();
    integer(ctx, result)
}

fn call_float(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> f64 {
    let result = call(runtime, ctx, name, args).unwrap();
    float(ctx, result)
}

fn call_ratio(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> (i128, i128) {
    let result = call(runtime, ctx, name, args).unwrap();
    ratio_parts(ctx, result)
}

#[test]
fn numeric_float_and_ratio_boundaries_have_specified_results() {
    let (runtime, mut ctx) = setup();
    let zero = Word::fixnum(0);
    let one = Word::fixnum(1);
    let minus_one = Word::fixnum(-1);
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(-6), Word::fixnum(4))
        .unwrap()
        .into();
    let positive = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    let negative = make_double(&mut ctx, &runtime, -2.5).unwrap().into();
    assert_eq!(
        call_ratio(&runtime, &mut ctx, "RATIONAL", &[positive]),
        (5, 2)
    );
    assert_eq!(
        call_ratio(&runtime, &mut ctx, "RATIONAL", &[negative]),
        (-5, 2)
    );
    assert_eq!(
        call_ratio(&runtime, &mut ctx, "RATIONALIZE", &[positive]),
        (5, 2)
    );
    assert_eq!(
        call_ratio(&runtime, &mut ctx, "RATIONALIZE", &[negative]),
        (-5, 2)
    );
    assert_eq!(call_integer(&runtime, &mut ctx, "NUMERATOR", &[ratio]), -6);
    assert_eq!(call_integer(&runtime, &mut ctx, "DENOMINATOR", &[ratio]), 4);
    assert_eq!(call_float(&runtime, &mut ctx, "FLOAT", &[ratio]), -1.5);
    assert_eq!(call_float(&runtime, &mut ctx, "FFLOOR", &[positive]), 2.0);
    assert_eq!(
        call_float(&runtime, &mut ctx, "FCEILING", &[negative]),
        -2.0
    );
    assert_eq!(
        call_float(&runtime, &mut ctx, "FTRUNCATE", &[negative]),
        -2.0
    );
    assert_eq!(call_float(&runtime, &mut ctx, "FROUND", &[positive]), 2.0);
    assert_eq!(call_float(&runtime, &mut ctx, "FROUND", &[negative]), -2.0);
    assert_eq!(
        call_integer(&runtime, &mut ctx, "+", &[zero, minus_one]),
        -1
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "*", &[minus_one, minus_one]),
        1
    );
    assert_eq!(call_integer(&runtime, &mut ctx, "ABS", &[minus_one]), 1);
    assert_eq!(call_integer(&runtime, &mut ctx, "SIGNUM", &[minus_one]), -1);
    assert_eq!(
        call_integer(&runtime, &mut ctx, "GCD", &[Word::fixnum(0), one]),
        1
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "LCM", &[Word::fixnum(0), one]),
        0
    );
}

#[test]
fn numeric_special_values_and_bit_boundaries_are_checked() {
    let (runtime, mut ctx) = setup();
    let huge = make_bignum_from_i128(&mut ctx, &runtime, i128::MAX)
        .unwrap()
        .into();
    let negative_huge = make_bignum_from_i128(&mut ctx, &runtime, i128::MIN + 1)
        .unwrap()
        .into();
    assert_eq!(call_integer(&runtime, &mut ctx, "LOGCOUNT", &[huge]), 127);
    assert_eq!(
        call_integer(&runtime, &mut ctx, "INTEGER-LENGTH", &[negative_huge]),
        127
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "LOGAND", &[huge, negative_huge]),
        1
    );
    assert_eq!(
        call_integer(
            &runtime,
            &mut ctx,
            "ASH",
            &[Word::fixnum(1), Word::fixnum(63)]
        ),
        1_i128 << 63
    );
    let spec = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(3), Word::fixnum(2)],
    )
    .unwrap();
    assert_eq!(
        call_integer(&runtime, &mut ctx, "LDB", &[spec, Word::fixnum(0b11100)]),
        7
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "MASK-FIELD", &[spec, Word::fixnum(-1)]),
        0b11100
    );
}

#[test]
fn predicates_cover_number_kinds_and_signs() {
    let (runtime, mut ctx) = setup();
    let float_value = make_double(&mut ctx, &runtime, -2.5).unwrap().into();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(3), Word::fixnum(2))
        .unwrap()
        .into();
    let real = make_double(&mut ctx, &runtime, 1.0).unwrap().into();
    let imag = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    let complex = make_complex(&mut ctx, &runtime, real, imag).unwrap().into();
    for (name, value, expected) in [
        ("NUMBERP", Word::fixnum(1), Word::TRUE),
        ("NUMBERP", Word::NIL, Word::NIL),
        ("INTEGERP", Word::fixnum(1), Word::TRUE),
        ("INTEGERP", ratio, Word::NIL),
        ("RATIONALP", ratio, Word::TRUE),
        ("RATIONALP", float_value, Word::NIL),
        ("FLOATP", float_value, Word::TRUE),
        ("REALP", ratio, Word::TRUE),
        ("REALP", complex, Word::NIL),
        ("COMPLEXP", complex, Word::TRUE),
        ("COMPLEXP", Word::fixnum(1), Word::NIL),
        ("ZEROP", Word::fixnum(0), Word::TRUE),
        ("ZEROP", Word::fixnum(-1), Word::NIL),
        ("PLUSP", Word::fixnum(2), Word::TRUE),
        ("PLUSP", Word::fixnum(-2), Word::NIL),
        ("MINUSP", Word::fixnum(-2), Word::TRUE),
        ("MINUSP", Word::fixnum(2), Word::NIL),
        ("EVENP", Word::fixnum(-4), Word::TRUE),
        ("ODDP", Word::fixnum(-3), Word::TRUE),
    ] {
        assert_eq!(
            call(&runtime, &mut ctx, name, &[value]),
            Ok(expected),
            "{name}"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "PLUSP", &[complex]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "MINUSP", &[complex]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn boole_and_logical_operations_return_actual_values() {
    let (runtime, mut ctx) = setup();
    let a = Word::fixnum(0b1010);
    let b = Word::fixnum(0b0110);
    for (opcode, expected) in [
        (0, 0),
        (1, !0b1110_i64),
        (2, !0b1010_i64 & !0b0110_i64),
        (3, !0b1010_i64),
        (4, 0b1010_i64 & !0b0110_i64),
        (5, !0b0110_i64),
        (6, 12),
        (7, !(0b1010_i64 & 0b0110_i64)),
        (8, 2),
        (9, !(0b1010_i64 ^ 0b0110_i64)),
        (10, 10),
        (11, 0b1010_i64 | !0b0110_i64),
        (12, 6),
        (13, !0b1010_i64 | 0b0110_i64),
        (14, 14),
        (15, -1),
    ] {
        assert_eq!(
            call_integer(&runtime, &mut ctx, "BOOLE", &[Word::fixnum(opcode), a, b]),
            i128::from(expected),
            "opcode {opcode}"
        );
    }
    assert_eq!(
        call(&runtime, &mut ctx, "BOOLE", &[Word::fixnum(16), a, b]),
        Err(ObjectError::TypeError)
    );
    for (name, args, expected) in [
        ("LOGAND", vec![], -1),
        ("LOGIOR", vec![], 0),
        ("LOGXOR", vec![], 0),
        ("LOGEQV", vec![], -1),
        ("LOGAND", vec![a, b], 2),
        ("LOGIOR", vec![a, b], 14),
        ("LOGXOR", vec![a, b], 12),
        ("LOGNAND", vec![a, b], -3),
        ("LOGNOR", vec![a, b], -15),
    ] {
        assert_eq!(
            call_integer(&runtime, &mut ctx, name, &args),
            expected,
            "{name}"
        );
    }
    assert_eq!(call_integer(&runtime, &mut ctx, "LOGNOT", &[a]), -11);
    assert_eq!(call(&runtime, &mut ctx, "LOGTEST", &[a, b]), Ok(Word::TRUE));
    assert_eq!(
        call_integer(&runtime, &mut ctx, "LOGCOUNT", &[Word::fixnum(-1)]),
        0
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "INTEGER-LENGTH", &[Word::fixnum(-8)]),
        3
    );
    assert_eq!(
        call_integer(
            &runtime,
            &mut ctx,
            "ASH",
            &[Word::fixnum(-8), Word::fixnum(-1)]
        ),
        -4
    );
}

#[test]
fn byte_fields_cover_boundaries_and_sign_extension() {
    let (runtime, mut ctx) = setup();
    let spec = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(4), Word::fixnum(3)],
    )
    .unwrap();
    assert_eq!(call_integer(&runtime, &mut ctx, "BYTE-SIZE", &[spec]), 4);
    assert_eq!(
        call_integer(&runtime, &mut ctx, "BYTE-POSITION", &[spec]),
        3
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "LDB", &[spec, Word::fixnum(0b101101)]),
        5
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "LDB-TEST",
            &[spec, Word::fixnum(0b10_00000)]
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LDB-TEST", &[spec, Word::fixnum(0)]),
        Ok(Word::NIL)
    );
    assert_eq!(
        call_integer(
            &runtime,
            &mut ctx,
            "DPB",
            &[Word::fixnum(2), spec, Word::fixnum(127)]
        ),
        23
    );
    assert_eq!(
        call_integer(
            &runtime,
            &mut ctx,
            "DEPOSIT-FIELD",
            &[Word::fixnum(0), spec, Word::fixnum(127)]
        ),
        7
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "MASK-FIELD", &[spec, Word::fixnum(-1)]),
        120
    );
    let wide = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(4), Word::fixnum(0)],
    )
    .unwrap();
    let min = make_bignum_from_i128(&mut ctx, &runtime, i128::MIN)
        .unwrap()
        .into();
    assert_eq!(call_integer(&runtime, &mut ctx, "LDB", &[wide, min]), 0);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "BYTE",
            &[Word::fixnum(-1), Word::fixnum(0)]
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn complex_accessors_and_constructors_preserve_values() {
    let (runtime, mut ctx) = setup();
    let real = make_double(&mut ctx, &runtime, 3.0).unwrap();
    let imag = make_double(&mut ctx, &runtime, -4.0).unwrap();
    let complex = make_complex(&mut ctx, &runtime, real.into(), imag.into())
        .unwrap()
        .into();
    assert_eq!(complex_parts(&ctx, complex), (3.0, -4.0));
    assert_eq!(call_float(&runtime, &mut ctx, "REALPART", &[complex]), 3.0);
    assert_eq!(call_float(&runtime, &mut ctx, "IMAGPART", &[complex]), -4.0);
    let conjugate = call(&runtime, &mut ctx, "CONJUGATE", &[complex]).unwrap();
    assert_eq!(complex_parts(&ctx, conjugate), (3.0, 4.0));
    assert_eq!(
        call_float(&runtime, &mut ctx, "REALPART", &[Word::fixnum(7)]),
        7.0
    );
    assert_eq!(
        call_float(&runtime, &mut ctx, "IMAGPART", &[Word::fixnum(7)]),
        0.0
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "COMPLEX",
            &[Word::fixnum(7), Word::fixnum(0)]
        ),
        Ok(Word::fixnum(7))
    );
    let cis = call(&runtime, &mut ctx, "CIS", &[Word::fixnum(0)]).unwrap();
    assert_eq!(complex_parts(&ctx, cis), (1.0, 0.0));
    assert!((call_float(&runtime, &mut ctx, "PHASE", &[complex]) + 0.927295218).abs() < 1e-8);
}

#[test]
fn rational_and_float_operations_cover_exact_values() {
    let (runtime, mut ctx) = setup();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(6), Word::fixnum(8))
        .unwrap()
        .into();
    assert_eq!(call_ratio(&runtime, &mut ctx, "RATIONAL", &[ratio]), (3, 4));
    assert_eq!(call_integer(&runtime, &mut ctx, "NUMERATOR", &[ratio]), 6);
    assert_eq!(call_integer(&runtime, &mut ctx, "DENOMINATOR", &[ratio]), 8);
    let value = make_double(&mut ctx, &runtime, 0.5).unwrap().into();
    assert_eq!(call_ratio(&runtime, &mut ctx, "RATIONAL", &[value]), (1, 2));
    assert_eq!(
        call_ratio(&runtime, &mut ctx, "RATIONALIZE", &[value]),
        (1, 2)
    );
    assert_eq!(call_float(&runtime, &mut ctx, "FLOAT", &[ratio]), 0.75);
    let negative = make_double(&mut ctx, &runtime, -3.0).unwrap().into();
    assert_eq!(
        call_float(&runtime, &mut ctx, "FLOAT-SIGN", &[negative, value]),
        -0.5
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "FLOAT-DIGITS", &[value]),
        53
    );
    assert_eq!(
        call_integer(&runtime, &mut ctx, "FLOAT-PRECISION", &[value]),
        53
    );
    assert_eq!(call_integer(&runtime, &mut ctx, "FLOAT-RADIX", &[value]), 2);
    assert_eq!(
        call_float(&runtime, &mut ctx, "SCALE-FLOAT", &[value, Word::fixnum(2)]),
        2.0
    );
    let decoded = call(&runtime, &mut ctx, "DECODE-FLOAT", &[value]).unwrap();
    assert_eq!(float(&ctx, decoded), 0.5);
    assert_eq!(ctx.values().len(), 3);
    let zero = make_double(&mut ctx, &runtime, 0.0).unwrap().into();
    assert_eq!(
        call_ratio(&runtime, &mut ctx, "RATIONALIZE", &[value, zero]),
        (1, 2)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "RATIONALIZE", &[Word::fixnum(1), value]),
        Ok(Word::fixnum(1))
    );
}

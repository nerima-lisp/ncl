#![allow(clippy::unwrap_used, missing_docs)]

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, double_value, make_complex, make_double, make_ratio,
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
    let function = FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", name)
            .expect("builtin exists"),
    )
    .unwrap();
    runtime.call_builtin(ctx, function, args)
}

fn integer(ctx: &ThreadContext, value: Word) -> i128 {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => i128::from(value),
        ObjectRef::Bignum(value) => {
            let limbs =
                ncl_object::bignum_limbs(ctx, ncl_object::Bignum::from_word(value)).unwrap();
            let magnitude = limbs
                .into_iter()
                .enumerate()
                .fold(0_u128, |value, (index, limb)| {
                    value | (u128::from(limb) << (index * 32))
                });
            if ncl_object::bignum_sign(ctx, ncl_object::Bignum::from_word(value)).unwrap() {
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
    match classify_object(ctx, value) {
        ObjectRef::DoubleFloat(value) => double_value(ctx, DoubleFloat::from_word(value)).unwrap(),
        other => panic!("expected float, got {other:?}"),
    }
}

fn ratio(ctx: &mut ThreadContext, runtime: &Runtime, numerator: i64, denominator: i64) -> Word {
    make_ratio(
        ctx,
        runtime,
        Word::fixnum(numerator),
        Word::fixnum(denominator),
    )
    .unwrap()
    .into()
}

fn assert_integer_call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: i128,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    assert_eq!(integer(ctx, result), expected, "{name}");
}

fn assert_float_call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: f64,
) {
    let result = call(runtime, ctx, name, args).unwrap();
    assert!((float(ctx, result) - expected).abs() < 1e-12, "{name}");
}

#[test]
fn integer_bit_operations_cover_signed_and_empty_fold_cases() {
    let (runtime, mut ctx) = setup();
    let a = Word::fixnum(0b1100);
    let b = Word::fixnum(0b1010);
    assert_integer_call(&runtime, &mut ctx, "LOGAND", &[], -1);
    assert_integer_call(&runtime, &mut ctx, "LOGIOR", &[], 0);
    assert_integer_call(&runtime, &mut ctx, "LOGXOR", &[a, b], 6);
    assert_integer_call(&runtime, &mut ctx, "LOGNOT", &[a], -13);
    assert_integer_call(&runtime, &mut ctx, "LOGEQV", &[a, b], -7);
    assert_integer_call(&runtime, &mut ctx, "LOGNAND", &[a, b], -9);
    assert_integer_call(&runtime, &mut ctx, "LOGNOR", &[a, b], -15);
    assert_integer_call(&runtime, &mut ctx, "LOGANDC1", &[a, b], 2);
    assert_integer_call(&runtime, &mut ctx, "LOGANDC2", &[a, b], 4);
    assert_integer_call(&runtime, &mut ctx, "LOGORC1", &[a, b], -5);
    assert_integer_call(&runtime, &mut ctx, "LOGORC2", &[a, b], -3);
    assert_eq!(
        call(&runtime, &mut ctx, "LOGTEST", &[a, b]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "LOGBITP", &[Word::fixnum(2), a]).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "LOGBITP",
            &[Word::fixnum(127), Word::fixnum(-1)]
        )
        .unwrap(),
        Word::TRUE
    );
    assert_integer_call(&runtime, &mut ctx, "LOGCOUNT", &[Word::fixnum(-9)], 1);
    assert_integer_call(&runtime, &mut ctx, "INTEGER-LENGTH", &[Word::fixnum(-9)], 4);
    assert_integer_call(&runtime, &mut ctx, "ASH", &[a, Word::fixnum(2)], 48);
    assert_integer_call(&runtime, &mut ctx, "ASH", &[a, Word::fixnum(-2)], 3);
    assert_eq!(
        call(&runtime, &mut ctx, "BOOLE", &[Word::fixnum(6), a, b])
            .unwrap()
            .as_fixnum(),
        Some(6)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "BOOLE", &[Word::fixnum(99), a, b]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn byte_field_operations_return_encoded_spec_and_updated_values() {
    let (runtime, mut ctx) = setup();
    let spec = call(
        &runtime,
        &mut ctx,
        "BYTE",
        &[Word::fixnum(4), Word::fixnum(3)],
    )
    .unwrap();
    assert_integer_call(&runtime, &mut ctx, "BYTE-SIZE", &[spec], 4);
    assert_integer_call(&runtime, &mut ctx, "BYTE-POSITION", &[spec], 3);
    assert_integer_call(
        &runtime,
        &mut ctx,
        "LDB",
        &[spec, Word::fixnum(0b101101)],
        5,
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "LDB-TEST",
            &[spec, Word::fixnum(0b101101)]
        )
        .unwrap(),
        Word::TRUE
    );
    assert_integer_call(
        &runtime,
        &mut ctx,
        "MASK-FIELD",
        &[spec, Word::fixnum(-1)],
        0b1111000,
    );
    assert_integer_call(
        &runtime,
        &mut ctx,
        "DPB",
        &[Word::fixnum(2), spec, Word::fixnum(0)],
        16,
    );
    assert_integer_call(
        &runtime,
        &mut ctx,
        "DEPOSIT-FIELD",
        &[Word::fixnum(0b1010), spec, Word::fixnum(0)],
        8,
    );
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
fn rational_float_builtins_assert_multiple_values_and_boundaries() {
    let (runtime, mut ctx) = setup();
    let r = ratio(&mut ctx, &runtime, 6, 4);
    assert_integer_call(&runtime, &mut ctx, "NUMERATOR", &[r], 6);
    assert_integer_call(&runtime, &mut ctx, "DENOMINATOR", &[r], 4);
    assert_integer_call(&runtime, &mut ctx, "NUMERATOR", &[Word::fixnum(7)], 7);
    assert_integer_call(&runtime, &mut ctx, "DENOMINATOR", &[Word::fixnum(7)], 1);

    let value = make_double(&mut ctx, &runtime, -1.5).unwrap().into();
    let rational = call(&runtime, &mut ctx, "RATIONAL", &[value]).unwrap();
    assert_eq!(
        integer(
            &ctx,
            ncl_object::ratio_numerator(&ctx, ncl_object::Ratio::from_word(rational)).unwrap()
        ),
        -3
    );
    assert_eq!(
        integer(
            &ctx,
            ncl_object::ratio_denominator(&ctx, ncl_object::Ratio::from_word(rational)).unwrap()
        ),
        2
    );
    assert_float_call(&runtime, &mut ctx, "FLOAT", &[r], 1.5);
    let sign = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    assert_float_call(&runtime, &mut ctx, "FLOAT-SIGN", &[value, sign], -2.0);
    assert_eq!(
        call(&runtime, &mut ctx, "RATIONAL", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );

    let decoded = call(&runtime, &mut ctx, "DECODE-FLOAT", &[value]).unwrap();
    assert!((float(&ctx, decoded) - 0.75).abs() < 1e-12);
    assert_eq!(integer(&ctx, ctx.values()[1]), 1);
    assert_eq!(float(&ctx, ctx.values()[2]), -1.0);
    let integer_decoded = call(&runtime, &mut ctx, "INTEGER-DECODE-FLOAT", &[value]).unwrap();
    assert_eq!(integer(&ctx, integer_decoded), 6755399441055744);
    assert_eq!(integer(&ctx, ctx.values()[1]), -52);
    assert_eq!(integer(&ctx, ctx.values()[2]), -1);
    let zero_float = make_double(&mut ctx, &runtime, 0.0).unwrap().into();
    assert_integer_call(&runtime, &mut ctx, "FLOAT-PRECISION", &[zero_float], 0);
    assert_integer_call(&runtime, &mut ctx, "FLOAT-DIGITS", &[value], 53);
    assert_integer_call(&runtime, &mut ctx, "FLOAT-RADIX", &[value], 2);
    assert_float_call(
        &runtime,
        &mut ctx,
        "SCALE-FLOAT",
        &[value, Word::fixnum(2)],
        -6.0,
    );
}

#[test]
fn complex_accessors_construct_conjugate_and_phase_values() {
    let (runtime, mut ctx) = setup();
    let real = make_double(&mut ctx, &runtime, 3.0).unwrap().into();
    let imag = make_double(&mut ctx, &runtime, 4.0).unwrap().into();
    let z = make_complex(&mut ctx, &runtime, real, imag).unwrap().into();
    let conjugate = call(&runtime, &mut ctx, "CONJUGATE", &[z]).unwrap();
    assert_float_call(&runtime, &mut ctx, "REALPART", &[conjugate], 3.0);
    assert_float_call(&runtime, &mut ctx, "IMAGPART", &[conjugate], -4.0);
    assert_float_call(&runtime, &mut ctx, "PHASE", &[z], 4.0_f64.atan2(3.0));
    let real_result = call(
        &runtime,
        &mut ctx,
        "COMPLEX",
        &[Word::fixnum(7), Word::fixnum(0)],
    )
    .unwrap();
    assert_eq!(real_result, Word::fixnum(7));
    let cis = call(&runtime, &mut ctx, "CIS", &[Word::fixnum(0)]).unwrap();
    assert!(matches!(classify_object(&ctx, cis), ObjectRef::Complex(_)));
    assert_eq!(
        call(&runtime, &mut ctx, "REALPART", &[Word::TRUE]),
        Err(ObjectError::TypeError)
    );
}

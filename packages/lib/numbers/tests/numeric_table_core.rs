#![allow(clippy::float_cmp, clippy::unwrap_used, missing_docs)]

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectRef, Runtime, ThreadContext, Word, classify_object,
    double_value, make_double, ratio_denominator, ratio_numerator,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let symbol = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    runtime
        .call_builtin(ctx, FunctionObject::try_from(symbol).unwrap(), args)
        .unwrap()
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
                -i128::try_from(magnitude).unwrap()
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

fn ratio(ctx: &ThreadContext, value: Word) -> (i128, i128) {
    let ObjectRef::Ratio(value) = classify_object(ctx, value) else {
        panic!("expected ratio")
    };
    let value = ncl_object::Ratio::from_word(value);
    (
        integer(ctx, ratio_numerator(ctx, value).unwrap()),
        integer(ctx, ratio_denominator(ctx, value).unwrap()),
    )
}

#[test]
fn core_table_asserts_exact_powers_and_float_fallbacks() {
    let (runtime, mut ctx) = setup();
    let exact = call(
        &runtime,
        &mut ctx,
        "EXPT",
        &[Word::fixnum(2), Word::fixnum(-3)],
    );
    assert_eq!(ratio(&ctx, exact), (1, 8));
    let fallback = call(
        &runtime,
        &mut ctx,
        "EXPT",
        &[Word::fixnum(2), Word::fixnum(127)],
    );
    assert_eq!(float(&ctx, fallback).to_bits(), 0x47df_ffff_ffff_ffd4);
    let one = make_double(&mut ctx, &runtime, 1.0).unwrap().into();
    let scaled = call(&runtime, &mut ctx, "SCALE-FLOAT", &[one, Word::fixnum(3)]);
    assert_eq!(float(&ctx, scaled), 8.0);
}

#[test]
fn rational_float_table_asserts_subnormal_decode_and_precision() {
    let (runtime, mut ctx) = setup();
    let smallest = make_double(&mut ctx, &runtime, f64::from_bits(1))
        .unwrap()
        .into();
    let significand = call(&runtime, &mut ctx, "DECODE-FLOAT", &[smallest]);
    assert_eq!(
        float(&ctx, significand).to_bits(),
        (2_f64.powi(-52)).to_bits()
    );
    assert_eq!(integer(&ctx, ctx.values()[1]), -1022);
    assert_eq!(float(&ctx, ctx.values()[2]), 1.0);
    let precision = call(&runtime, &mut ctx, "FLOAT-PRECISION", &[smallest]);
    assert_eq!(integer(&ctx, precision), 1);
    let decoded = call(&runtime, &mut ctx, "INTEGER-DECODE-FLOAT", &[smallest]);
    assert_eq!(integer(&ctx, decoded), 1);
    assert_eq!(integer(&ctx, ctx.values()[1]), -1074);
    assert_eq!(integer(&ctx, ctx.values()[2]), 1);
}

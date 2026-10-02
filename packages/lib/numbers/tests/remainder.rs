#![allow(
    clippy::float_cmp,
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert exact numeric builtin behavior"
)]

use ncl_object::{
    FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word, classify_object,
    double_value, make_double, make_ratio, ratio_denominator, ratio_numerator,
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
        other => panic!("expected fixnum, got {other:?}"),
    }
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

#[test]
fn mod_and_rem_preserve_common_lisp_sign_rules() {
    let (runtime, mut ctx) = setup();
    assert_integer_call(
        &runtime,
        &mut ctx,
        "MOD",
        &[Word::fixnum(-7), Word::fixnum(2)],
        1,
    );
    assert_integer_call(
        &runtime,
        &mut ctx,
        "REM",
        &[Word::fixnum(-7), Word::fixnum(2)],
        -1,
    );
    assert_integer_call(
        &runtime,
        &mut ctx,
        "MOD",
        &[Word::fixnum(7), Word::fixnum(-2)],
        -1,
    );
}

#[test]
fn remainder_rejects_zero_divisor() {
    let (runtime, mut ctx) = setup();
    for name in ["MOD", "REM"] {
        assert_eq!(
            call(
                &runtime,
                &mut ctx,
                name,
                &[Word::fixnum(5), Word::fixnum(0)]
            ),
            Err(ObjectError::TypeError),
            "{name}"
        );
    }
}

#[test]
fn gcd_lcm_and_isqrt_cover_identity_and_boundaries() {
    let (runtime, mut ctx) = setup();
    assert_integer_call(&runtime, &mut ctx, "GCD", &[], 0);
    assert_integer_call(
        &runtime,
        &mut ctx,
        "GCD",
        &[Word::fixnum(-18), Word::fixnum(24)],
        6,
    );
    assert_integer_call(&runtime, &mut ctx, "LCM", &[], 1);
    assert_integer_call(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(-6), Word::fixnum(8)],
        24,
    );
    assert_integer_call(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(15)], 3);
    assert_integer_call(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(16)], 4);
    assert_eq!(
        call(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(-1)]),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn remainder_handles_ratios_floats_and_zero_lcm_inputs() {
    let (runtime, mut ctx) = setup();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(7), Word::fixnum(3))
        .unwrap()
        .into();
    let divisor = make_ratio(&mut ctx, &runtime, Word::fixnum(2), Word::fixnum(3))
        .unwrap()
        .into();
    let result = call(&runtime, &mut ctx, "MOD", &[ratio, divisor]).unwrap();
    let ObjectRef::Ratio(value) = classify_object(&ctx, result) else {
        panic!("expected ratio remainder");
    };
    let value = ncl_object::Ratio::from_word(value);
    assert_eq!(integer(&ctx, ratio_numerator(&ctx, value).unwrap()), 1);
    assert_eq!(integer(&ctx, ratio_denominator(&ctx, value).unwrap()), 3);

    let value = make_double(&mut ctx, &runtime, -7.5).unwrap().into();
    let divisor = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    let result = call(&runtime, &mut ctx, "REM", &[value, divisor]).unwrap();
    let ObjectRef::DoubleFloat(result) = classify_object(&ctx, result) else {
        panic!("expected float remainder");
    };
    assert_eq!(
        double_value(&ctx, ncl_object::DoubleFloat::from_word(result)).unwrap(),
        -1.5
    );
    assert_integer_call(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(0), Word::fixnum(9)],
        0,
    );
}

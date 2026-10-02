#![allow(clippy::float_cmp, clippy::unwrap_used, missing_docs)]

use ncl_object::{
    FunctionObject, ObjectError, ObjectRef, Runtime, ThreadContext, Word, make_bignum_from_i128,
    make_complex, make_double, make_ratio,
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

fn complex(ctx: &mut ThreadContext, runtime: &Runtime, real: f64, imag: f64) -> Word {
    let real = make_double(ctx, runtime, real).unwrap().into();
    let imag = make_double(ctx, runtime, imag).unwrap().into();
    make_complex(ctx, runtime, real, imag).unwrap().into()
}

fn expect_type_error(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) {
    assert_eq!(
        call(runtime, ctx, name, args),
        Err(ObjectError::TypeError),
        "{name} must report TypeError"
    );
}

#[test]
fn complex_error_contracts_reject_non_real_components_and_bad_arity() {
    let (runtime, mut ctx) = setup();
    let half = ratio(&mut ctx, &runtime, 1, 2);
    let large = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 70)
        .unwrap()
        .into();
    let z = complex(&mut ctx, &runtime, 3.0, -4.0);

    for name in ["REALPART", "PHASE", "CIS"] {
        expect_type_error(&runtime, &mut ctx, name, &[half]);
        expect_type_error(&runtime, &mut ctx, name, &[large]);
        expect_type_error(&runtime, &mut ctx, name, &[Word::TRUE]);
    }
    for value in [half, large, Word::TRUE] {
        let result = call(&runtime, &mut ctx, "IMAGPART", &[value]).unwrap();
        let ObjectRef::DoubleFloat(result) = ncl_object::classify_object(&ctx, result) else {
            panic!("IMAGPART must return a double-float")
        };
        assert_eq!(
            ncl_object::double_value(&ctx, ncl_object::DoubleFloat::from_word(result)).unwrap(),
            0.0
        );
    }
    for name in ["CONJUGATE", "REALPART", "IMAGPART", "PHASE", "CIS"] {
        expect_type_error(&runtime, &mut ctx, name, &[]);
        expect_type_error(&runtime, &mut ctx, name, &[z, Word::fixnum(1)]);
    }
    expect_type_error(&runtime, &mut ctx, "COMPLEX", &[]);
    expect_type_error(&runtime, &mut ctx, "COMPLEX", &[Word::fixnum(1)]);
    expect_type_error(
        &runtime,
        &mut ctx,
        "COMPLEX",
        &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
    );
    expect_type_error(&runtime, &mut ctx, "COMPLEX", &[Word::fixnum(1), half]);
}

#[test]
fn complex_division_errors_preserve_zero_and_non_numeric_contracts() {
    let (runtime, mut ctx) = setup();
    let z = complex(&mut ctx, &runtime, 3.0, -4.0);
    let zero = complex(&mut ctx, &runtime, 0.0, 0.0);
    let nonzero = complex(&mut ctx, &runtime, 1.0, 0.0);

    for args in [[z, Word::fixnum(0)], [z, zero], [Word::fixnum(1), zero]] {
        expect_type_error(&runtime, &mut ctx, "/", &args);
    }
    expect_type_error(&runtime, &mut ctx, "/", &[Word::TRUE, nonzero]);
    expect_type_error(&runtime, &mut ctx, "/", &[nonzero, Word::TRUE]);
    expect_type_error(&runtime, &mut ctx, "EXPT", &[Word::TRUE, Word::fixnum(2)]);
    expect_type_error(&runtime, &mut ctx, "EXPT", &[Word::fixnum(2), Word::TRUE]);
}

#[test]
fn remainder_error_contracts_reject_non_integer_inputs_and_missing_operands() {
    let (runtime, mut ctx) = setup();
    let ratio_value = ratio(&mut ctx, &runtime, 3, 2);
    let float_value = make_double(&mut ctx, &runtime, 3.5).unwrap().into();
    let complex_value = complex(&mut ctx, &runtime, 1.0, 2.0);

    for name in ["MOD", "REM"] {
        expect_type_error(&runtime, &mut ctx, name, &[]);
        expect_type_error(&runtime, &mut ctx, name, &[Word::fixnum(1)]);
        expect_type_error(&runtime, &mut ctx, name, &[ratio_value, Word::TRUE]);
        expect_type_error(&runtime, &mut ctx, name, &[float_value, Word::TRUE]);
        expect_type_error(&runtime, &mut ctx, name, &[complex_value, Word::fixnum(2)]);
        expect_type_error(
            &runtime,
            &mut ctx,
            name,
            &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
        );
    }
    for name in ["GCD", "LCM"] {
        expect_type_error(&runtime, &mut ctx, name, &[ratio_value]);
        expect_type_error(&runtime, &mut ctx, name, &[float_value]);
        expect_type_error(&runtime, &mut ctx, name, &[complex_value]);
    }
    for value in [ratio_value, float_value, complex_value, Word::TRUE] {
        expect_type_error(&runtime, &mut ctx, "ISQRT", &[value]);
    }
    expect_type_error(&runtime, &mut ctx, "ISQRT", &[]);
    expect_type_error(
        &runtime,
        &mut ctx,
        "ISQRT",
        &[Word::fixnum(1), Word::fixnum(2)],
    );
    let minimum = make_bignum_from_i128(&mut ctx, &runtime, i128::MIN)
        .unwrap()
        .into();
    expect_type_error(&runtime, &mut ctx, "LCM", &[minimum]);
}

#[test]
fn rounding_error_contracts_reject_bad_arity_and_non_numeric_divisors() {
    let (runtime, mut ctx) = setup();
    let names = [
        "FLOOR",
        "CEILING",
        "TRUNCATE",
        "ROUND",
        "FFLOOR",
        "FCEILING",
        "FTRUNCATE",
        "FROUND",
    ];
    for name in names {
        expect_type_error(&runtime, &mut ctx, name, &[]);
        expect_type_error(&runtime, &mut ctx, name, &[Word::fixnum(1), Word::TRUE]);
        expect_type_error(
            &runtime,
            &mut ctx,
            name,
            &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
        );
        expect_type_error(&runtime, &mut ctx, name, &[Word::fixnum(1), Word::NIL]);
    }
    let ratio_value = ratio(&mut ctx, &runtime, 3, 2);
    for name in names {
        expect_type_error(&runtime, &mut ctx, name, &[ratio_value, Word::TRUE]);
    }
}

#[test]
fn random_error_contracts_reject_invalid_state_shapes_and_limits() {
    let (runtime, mut ctx) = setup();
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[]).unwrap();
    let wrong_instance = ncl_object::make_instance(&mut ctx, &runtime, Word::fixnum(99), &[])
        .unwrap()
        .into();

    expect_type_error(
        &runtime,
        &mut ctx,
        "RANDOM",
        &[Word::fixnum(10), Word::TRUE],
    );
    expect_type_error(
        &runtime,
        &mut ctx,
        "RANDOM",
        &[Word::fixnum(10), wrong_instance],
    );
    expect_type_error(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(-1), state]);
    expect_type_error(&runtime, &mut ctx, "RANDOM", &[Word::NIL, state]);
    expect_type_error(
        &runtime,
        &mut ctx,
        "RANDOM",
        &[Word::fixnum(10), state, Word::NIL],
    );
    expect_type_error(&runtime, &mut ctx, "RANDOM", &[]);
    expect_type_error(&runtime, &mut ctx, "RANDOM-STATE-P", &[]);
    expect_type_error(&runtime, &mut ctx, "RANDOM-STATE-P", &[state, Word::NIL]);
    expect_type_error(
        &runtime,
        &mut ctx,
        "MAKE-RANDOM-STATE",
        &[Word::NIL, Word::TRUE],
    );
}

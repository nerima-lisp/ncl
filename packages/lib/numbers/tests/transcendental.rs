#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "numeric builtin assertions"
)]

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, classify_object, complex_imag, complex_real,
    double_value, make_complex, make_double,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    runtime
        .call_builtin(
            ctx,
            ncl_object::FunctionObject::try_from(function).unwrap(),
            args,
        )
        .unwrap()
}

fn real(ctx: &ThreadContext, value: Word) -> f64 {
    let ObjectRef::DoubleFloat(value) = classify_object(ctx, value) else {
        panic!("expected double-float")
    };
    double_value(ctx, ncl_object::DoubleFloat::from_word(value)).unwrap()
}

fn pair(ctx: &ThreadContext, value: Word) -> (f64, f64) {
    let ObjectRef::Complex(value) = classify_object(ctx, value) else {
        panic!("expected complex")
    };
    let value = ncl_object::Complex::from_word(value);
    let real_value = complex_real(ctx, value).unwrap();
    let imag_value = complex_imag(ctx, value).unwrap();
    (real(ctx, real_value), real(ctx, imag_value))
}

fn complex(ctx: &mut ThreadContext, runtime: &Runtime, real: f64, imag: f64) -> Word {
    let real = make_double(ctx, runtime, real).unwrap().into();
    let imag = make_double(ctx, runtime, imag).unwrap().into();
    make_complex(ctx, runtime, real, imag).unwrap().into()
}

fn close(actual: f64, expected: f64) {
    let tolerance = 2.0e-13_f64.max(2.0e-13 * expected.abs());
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}"
    );
}

fn close_pair(actual: (f64, f64), expected: (f64, f64)) {
    close(actual.0, expected.0);
    close(actual.1, expected.1);
}

fn call_real(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> f64 {
    let result = call(runtime, ctx, name, args);
    real(ctx, result)
}

fn call_pair(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> (f64, f64) {
    let result = call(runtime, ctx, name, args);
    pair(ctx, result)
}

#[test]
fn real_functions_match_f64_with_optional_log_and_atan() {
    let (runtime, mut ctx) = setup();
    let one = Word::fixnum(1);
    close(call_real(&runtime, &mut ctx, "EXP", &[one]), 1.0_f64.exp());
    close(
        call_real(
            &runtime,
            &mut ctx,
            "EXPT",
            &[Word::fixnum(2), Word::fixnum(10)],
        ),
        1024.0,
    );
    close(
        call_real(
            &runtime,
            &mut ctx,
            "LOG",
            &[Word::fixnum(8), Word::fixnum(2)],
        ),
        3.0,
    );
    close(
        call_real(&runtime, &mut ctx, "SQRT", &[Word::fixnum(9)]),
        3.0,
    );
    let half = make_double(&mut ctx, &runtime, 0.5).unwrap().into();
    for (name, expected) in [
        ("SIN", 0.5_f64.sin()),
        ("COS", 0.5_f64.cos()),
        ("TAN", 0.5_f64.tan()),
        ("SINH", 0.5_f64.sinh()),
        ("COSH", 0.5_f64.cosh()),
        ("TANH", 0.5_f64.tanh()),
        ("ASIN", 0.5_f64.asin()),
        ("ACOS", 0.5_f64.acos()),
        ("ATAN", 0.5_f64.atan()),
    ] {
        close(call_real(&runtime, &mut ctx, name, &[half]), expected);
    }
    close(
        call_real(
            &runtime,
            &mut ctx,
            "ATAN",
            &[Word::fixnum(1), Word::fixnum(1)],
        ),
        std::f64::consts::FRAC_PI_4,
    );
}

#[test]
fn complex_branches_and_inverse_hyperbolics() {
    let (runtime, mut ctx) = setup();
    let z = complex(&mut ctx, &runtime, 0.3, -0.7);
    let expected = (0.3_f64, -0.7_f64);
    close_pair(
        call_pair(&runtime, &mut ctx, "EXP", &[z]),
        (
            expected.0.exp() * expected.1.cos(),
            expected.0.exp() * expected.1.sin(),
        ),
    );
    close_pair(
        call_pair(&runtime, &mut ctx, "SIN", &[z]),
        (
            expected.0.sin() * expected.1.cosh(),
            expected.0.cos() * expected.1.sinh(),
        ),
    );
    close_pair(
        call_pair(&runtime, &mut ctx, "COS", &[z]),
        (
            expected.0.cos() * expected.1.cosh(),
            -expected.0.sin() * expected.1.sinh(),
        ),
    );
    for name in [
        "TAN", "SINH", "COSH", "TANH", "ASIN", "ACOS", "ATAN", "ASINH", "ACOSH", "ATANH",
    ] {
        let result = call_pair(&runtime, &mut ctx, name, &[z]);
        assert!(result.0.is_finite() && result.1.is_finite());
    }
    let negative_one = complex(&mut ctx, &runtime, -1.0, 0.0);
    close_pair(
        call_pair(&runtime, &mut ctx, "SQRT", &[negative_one]),
        (0.0, 1.0),
    );
}

#[test]
fn signed_zero_infinities_and_real_domain_boundaries() {
    let (runtime, mut ctx) = setup();
    let negative_zero = make_double(&mut ctx, &runtime, -0.0).unwrap().into();
    let result = call_real(&runtime, &mut ctx, "SIN", &[negative_zero]);
    assert_eq!(result.to_bits(), (-0.0_f64).to_bits());
    let infinity = make_double(&mut ctx, &runtime, f64::INFINITY)
        .unwrap()
        .into();
    assert!(call_real(&runtime, &mut ctx, "EXP", &[infinity]).is_infinite());
    assert!(call_real(&runtime, &mut ctx, "LOG", &[infinity]).is_infinite());
    let negative = call(&runtime, &mut ctx, "LOG", &[Word::fixnum(-1)]);
    let negative = pair(&ctx, negative);
    assert_eq!(negative.0.to_bits(), 0.0_f64.to_bits());
    close(negative.1, std::f64::consts::PI);
    let root = call_pair(&runtime, &mut ctx, "SQRT", &[Word::fixnum(-1)]);
    close_pair(root, (0.0, 1.0));
}

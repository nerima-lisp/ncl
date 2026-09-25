#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "numeric builtin assertions"
)]

use ncl_object::{
    classify_object, complex_imag, complex_real, double_value, ObjectRef, Runtime, ThreadContext,
    Word,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function =
        ncl_object::FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap())
            .unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn float(ctx: &ThreadContext, value: Word) -> f64 {
    let ObjectRef::DoubleFloat(value) = classify_object(ctx, value) else {
        panic!("expected double-float")
    };
    double_value(ctx, ncl_object::DoubleFloat::from_word(value)).unwrap()
}

fn close(left: f64, right: f64) {
    assert!((left - right).abs() < 1.0e-12, "{left} != {right}");
}

#[test]
fn real_transcendentals_and_optional_arguments() {
    let (runtime, mut ctx) = setup();
    let exp = call(&runtime, &mut ctx, "EXP", &[Word::fixnum(1)]);
    close(float(&ctx, exp), 1.0_f64.exp());
    let sqrt = call(&runtime, &mut ctx, "SQRT", &[Word::fixnum(9)]);
    close(float(&ctx, sqrt), 3.0);
    let log = call(&runtime, &mut ctx, "LOG", &[Word::fixnum(8), Word::fixnum(2)]);
    close(float(&ctx, log), 3.0);
    let atan = call(&runtime, &mut ctx, "ATAN", &[Word::fixnum(1), Word::fixnum(1)]);
    close(float(&ctx, atan), std::f64::consts::FRAC_PI_4);
    let sinh = call(&runtime, &mut ctx, "SINH", &[Word::fixnum(0)]);
    close(float(&ctx, sinh), 0.0);
}

#[test]
fn complex_components_and_complex_functions() {
    let (runtime, mut ctx) = setup();
    let value = call(
        &runtime,
        &mut ctx,
        "COMPLEX",
        &[Word::fixnum(3), Word::fixnum(4)],
    );
    let conjugate = call(&runtime, &mut ctx, "CONJUGATE", &[value]);
    let ObjectRef::Complex(conjugate) = classify_object(&ctx, conjugate) else {
        panic!("expected complex")
    };
    let conjugate_real = complex_real(&ctx, ncl_object::Complex::from_word(conjugate)).unwrap();
    close(float(&ctx, conjugate_real), 3.0);
    let conjugate_imag = complex_imag(&ctx, ncl_object::Complex::from_word(conjugate)).unwrap();
    close(float(&ctx, conjugate_imag), -4.0);
    let realpart = call(&runtime, &mut ctx, "REALPART", &[value]);
    close(float(&ctx, realpart), 3.0);
    let imagpart = call(&runtime, &mut ctx, "IMAGPART", &[value]);
    close(float(&ctx, imagpart), 4.0);
    let cis = call(&runtime, &mut ctx, "CIS", &[Word::fixnum(0)]);
    let ObjectRef::Complex(cis) = classify_object(&ctx, cis) else {
        panic!("expected complex")
    };
    close(
        float(
            &ctx,
            complex_real(&ctx, ncl_object::Complex::from_word(cis)).unwrap(),
        ),
        1.0,
    );
    close(
        float(
            &ctx,
            complex_imag(&ctx, ncl_object::Complex::from_word(cis)).unwrap(),
        ),
        0.0,
    );
}

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
    close(
        float(&ctx, call(&runtime, &mut ctx, "EXP", &[Word::fixnum(1)])),
        1.0_f64.exp(),
    );
    close(
        float(&ctx, call(&runtime, &mut ctx, "SQRT", &[Word::fixnum(9)])),
        3.0,
    );
    close(
        float(
            &ctx,
            call(
                &runtime,
                &mut ctx,
                "LOG",
                &[Word::fixnum(8), Word::fixnum(2)],
            ),
        ),
        3.0,
    );
    close(
        float(
            &ctx,
            call(
                &runtime,
                &mut ctx,
                "ATAN",
                &[Word::fixnum(1), Word::fixnum(1)],
            ),
        ),
        std::f64::consts::FRAC_PI_4,
    );
    close(
        float(&ctx, call(&runtime, &mut ctx, "SINH", &[Word::fixnum(0)])),
        0.0,
    );
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
    close(
        float(
            &ctx,
            complex_real(&ctx, ncl_object::Complex::from_word(conjugate)).unwrap(),
        ),
        3.0,
    );
    close(
        float(
            &ctx,
            complex_imag(&ctx, ncl_object::Complex::from_word(conjugate)).unwrap(),
        ),
        -4.0,
    );
    close(
        float(&ctx, call(&runtime, &mut ctx, "REALPART", &[value])),
        3.0,
    );
    close(
        float(&ctx, call(&runtime, &mut ctx, "IMAGPART", &[value])),
        4.0,
    );
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

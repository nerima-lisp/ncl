#![allow(clippy::float_cmp, clippy::unwrap_used, missing_docs)]

use ncl_object::{
    DoubleFloat, FunctionObject, ObjectRef, Runtime, ThreadContext, Word, classify_object,
    complex_imag, complex_real, double_value, make_complex, make_double,
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

fn float(ctx: &ThreadContext, value: Word) -> f64 {
    let ObjectRef::DoubleFloat(value) = classify_object(ctx, value) else {
        panic!("expected double-float")
    };
    double_value(ctx, DoubleFloat::from_word(value)).unwrap()
}

fn complex(ctx: &mut ThreadContext, runtime: &Runtime, real: f64, imag: f64) -> Word {
    let real = make_double(ctx, runtime, real).unwrap().into();
    let imag = make_double(ctx, runtime, imag).unwrap().into();
    make_complex(ctx, runtime, real, imag).unwrap().into()
}

#[test]
fn special_table_asserts_optional_transcendentals_and_components() {
    let (runtime, mut ctx) = setup();
    let log = call(
        &runtime,
        &mut ctx,
        "LOG",
        &[Word::fixnum(8), Word::fixnum(2)],
    );
    assert_eq!(float(&ctx, log), 3.0);
    let atan = call(
        &runtime,
        &mut ctx,
        "ATAN",
        &[Word::fixnum(1), Word::fixnum(1)],
    );
    assert_eq!(
        float(&ctx, atan).to_bits(),
        std::f64::consts::FRAC_PI_4.to_bits()
    );
    let realpart = call(&runtime, &mut ctx, "REALPART", &[Word::fixnum(3)]);
    let imagpart = call(&runtime, &mut ctx, "IMAGPART", &[Word::fixnum(3)]);
    assert_eq!(float(&ctx, realpart), 3.0);
    assert_eq!(float(&ctx, imagpart), 0.0);
    let root = call(&runtime, &mut ctx, "SQRT", &[Word::fixnum(-1)]);
    let ObjectRef::Complex(root) = classify_object(&ctx, root) else {
        panic!("expected complex square root")
    };
    let root = ncl_object::Complex::from_word(root);
    assert_eq!(float(&ctx, complex_real(&ctx, root).unwrap()), 0.0);
    assert_eq!(float(&ctx, complex_imag(&ctx, root).unwrap()), 1.0);
}

#[test]
fn special_table_asserts_complex_construction_and_float_rounding() {
    let (runtime, mut ctx) = setup();
    let z = complex(&mut ctx, &runtime, 3.0, 4.0);
    let conjugate = call(&runtime, &mut ctx, "CONJUGATE", &[z]);
    let ObjectRef::Complex(conjugate) = classify_object(&ctx, conjugate) else {
        panic!("expected complex conjugate")
    };
    let conjugate = ncl_object::Complex::from_word(conjugate);
    assert_eq!(float(&ctx, complex_real(&ctx, conjugate).unwrap()), 3.0);
    assert_eq!(float(&ctx, complex_imag(&ctx, conjugate).unwrap()), -4.0);
    let value = make_double(&mut ctx, &runtime, 2.5).unwrap().into();
    let rounded = call(&runtime, &mut ctx, "ROUND", &[value]);
    assert_eq!(float(&ctx, rounded), 2.0);
    assert_eq!(float(&ctx, ctx.values()[1]), 0.5);
}

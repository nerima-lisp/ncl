#![allow(clippy::unwrap_used, missing_docs)]

#[path = "../src/complex.rs"]
mod complex;
#[path = "../src/constants.rs"]
mod constants;
#[path = "../src/rational_float.rs"]
mod rational_float;

use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectRef, Runtime, ThreadContext, Word, classify_object,
    double_value, make_complex, make_double,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(
    f: ncl_object::RustBuiltin,
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
) -> Word {
    let mut values = MultipleValues::new();
    f(ctx, runtime, &BuiltinArgs::new(args), &mut values).unwrap()
}

#[test]
fn binary64_helpers_keep_explicit_boundaries() {
    let (runtime, mut ctx) = setup();
    let value = make_double(&mut ctx, &runtime, 1.5).unwrap().into();
    let mut values = MultipleValues::new();
    let result =
        rational_float::decode_float(&mut ctx, &runtime, &BuiltinArgs::new(&[value]), &mut values)
            .unwrap();
    assert!(
        (double_value(
            &ctx,
            ncl_object::DoubleFloat::from_word(values.as_slice()[0])
        )
        .unwrap()
            - 0.75)
            .abs()
            < 1e-15
    );
    assert_eq!(result, values.as_slice()[0]);
    assert_eq!(values.as_slice()[1], Word::fixnum(1));
    let scaled = call(
        rational_float::scale_float,
        &runtime,
        &mut ctx,
        &[value, Word::fixnum(2)],
    );
    assert!(
        (double_value(&ctx, ncl_object::DoubleFloat::from_word(scaled)).unwrap() - 6.0).abs()
            < 1e-15
    );
    assert_eq!(
        call(rational_float::float_digits, &runtime, &mut ctx, &[value]),
        Word::fixnum(53)
    );
    assert_eq!(
        call(rational_float::float_radix, &runtime, &mut ctx, &[value]),
        Word::fixnum(2)
    );
}

#[test]
fn complex_helpers_return_zero_imaginary_and_conjugate() {
    let (runtime, mut ctx) = setup();
    let real = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    let imag = make_double(&mut ctx, &runtime, 3.0).unwrap().into();
    let complex = make_complex(&mut ctx, &runtime, real, imag).unwrap().into();
    let result = call(complex::typed_conjugate, &runtime, &mut ctx, &[complex]);
    assert!(matches!(
        classify_object(&ctx, result),
        ObjectRef::Complex(_)
    ));
    let imag_part = call(complex::typed_imagpart, &runtime, &mut ctx, &[result]);
    assert!(
        (double_value(&ctx, ncl_object::DoubleFloat::from_word(imag_part)).unwrap() + 3.0).abs()
            < 1e-15
    );
    let zero = call(complex::typed_imagpart, &runtime, &mut ctx, &[real]);
    assert_eq!(
        double_value(&ctx, ncl_object::DoubleFloat::from_word(zero)).unwrap(),
        0.0
    );
}

#[test]
fn constants_register_with_binary64_values() {
    let (runtime, mut ctx) = setup();
    constants::register(&mut ctx, &runtime).unwrap();
    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let symbol = ncl_object::Package::from_word(package)
        .intern(&mut ctx, &runtime, "PI")
        .unwrap()
        .0;
    let value = ncl_object::symbol_value(&ctx, symbol).unwrap();
    assert!(
        (double_value(&ctx, ncl_object::DoubleFloat::from_word(value)).unwrap()
            - std::f64::consts::PI)
            .abs()
            < 1e-15
    );
}

#![allow(clippy::unwrap_used, missing_docs)]

use ncl_object::{
    FunctionObject, ObjectError, ObjectRef, Ratio, Runtime, ThreadContext, Word, classify_object,
    double_value, make_double, ratio_denominator, ratio_numerator,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
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

fn assert_ratio(ctx: &ThreadContext, value: Word, numerator: i64, denominator: i64) {
    let ObjectRef::Ratio(ratio) = classify_object(ctx, value) else {
        panic!("expected ratio, got {:?}", classify_object(ctx, value));
    };
    let ratio = Ratio::from_word(ratio);
    assert_eq!(
        ratio_numerator(ctx, ratio).unwrap().as_fixnum(),
        Some(numerator)
    );
    assert_eq!(
        ratio_denominator(ctx, ratio).unwrap().as_fixnum(),
        Some(denominator)
    );
}

fn double(ctx: &ThreadContext, value: Word) -> f64 {
    double_value(ctx, ncl_object::DoubleFloat::from_word(value)).unwrap()
}

#[test]
fn rational_builtins_keep_exact_and_tolerant_float_results() {
    let (runtime, mut ctx) = setup();
    let mut tenth = make_double(&mut ctx, &runtime, 0.1).unwrap().into();
    let tenth_root = ncl_object::push_root(&mut ctx, &mut tenth);

    let exact = call(&runtime, &mut ctx, "RATIONAL", &[tenth]).unwrap();
    assert_ratio(&ctx, exact, 3_602_879_701_896_397, 36_028_797_018_963_968);

    let mut tolerance = make_double(&mut ctx, &runtime, 0.01).unwrap().into();
    let tolerance_root = ncl_object::push_root(&mut ctx, &mut tolerance);
    let approximated = call(&runtime, &mut ctx, "RATIONALIZE", &[tenth, tolerance]).unwrap();
    assert_ratio(&ctx, approximated, 1, 10);
    assert!(ncl_object::pop_root(&mut ctx, tolerance_root));
    assert!(ncl_object::pop_root(&mut ctx, tenth_root));
}

#[test]
fn rational_builtins_reject_nonfinite_values_and_negative_tolerance() {
    let (runtime, mut ctx) = setup();
    for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let mut input = make_double(&mut ctx, &runtime, value).unwrap().into();
        let input_root = ncl_object::push_root(&mut ctx, &mut input);
        assert_eq!(
            call(&runtime, &mut ctx, "RATIONAL", &[input]),
            Err(ObjectError::TypeError),
            "RATIONAL({value})"
        );
        assert_eq!(
            call(&runtime, &mut ctx, "RATIONALIZE", &[input]),
            Err(ObjectError::TypeError),
            "RATIONALIZE({value})"
        );
        assert!(ncl_object::pop_root(&mut ctx, input_root));
    }

    let mut tenth = make_double(&mut ctx, &runtime, 0.1).unwrap().into();
    let tenth_root = ncl_object::push_root(&mut ctx, &mut tenth);
    let mut negative_tolerance = make_double(&mut ctx, &runtime, -0.01).unwrap().into();
    let tolerance_root = ncl_object::push_root(&mut ctx, &mut negative_tolerance);
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "RATIONALIZE",
            &[tenth, negative_tolerance]
        ),
        Err(ObjectError::TypeError)
    );
    assert!(ncl_object::pop_root(&mut ctx, tolerance_root));
    assert!(ncl_object::pop_root(&mut ctx, tenth_root));
}

#[test]
fn complex_builtins_preserve_real_identity_and_conjugate_components() {
    let (runtime, mut ctx) = setup();
    let real = Word::fixnum(3);
    let zero = Word::fixnum(0);
    let collapsed = call(&runtime, &mut ctx, "COMPLEX", &[real, zero]).unwrap();
    assert_eq!(collapsed, real);

    let real_part = call(&runtime, &mut ctx, "REALPART", &[collapsed]).unwrap();
    assert_eq!(double(&ctx, real_part), 3.0);
    let imaginary_part = call(&runtime, &mut ctx, "IMAGPART", &[collapsed]).unwrap();
    assert_eq!(double(&ctx, imaginary_part), 0.0);

    let mut real = make_double(&mut ctx, &runtime, 3.0).unwrap().into();
    let real_root = ncl_object::push_root(&mut ctx, &mut real);
    let mut imaginary = make_double(&mut ctx, &runtime, 4.0).unwrap().into();
    let imaginary_root = ncl_object::push_root(&mut ctx, &mut imaginary);
    let mut complex = call(&runtime, &mut ctx, "COMPLEX", &[real, imaginary]).unwrap();
    let complex_root = ncl_object::push_root(&mut ctx, &mut complex);

    let mut conjugate = call(&runtime, &mut ctx, "CONJUGATE", &[complex]).unwrap();
    let conjugate_root = ncl_object::push_root(&mut ctx, &mut conjugate);
    assert!(matches!(
        classify_object(&ctx, conjugate),
        ObjectRef::Complex(_)
    ));
    let real_part = call(&runtime, &mut ctx, "REALPART", &[conjugate]).unwrap();
    assert_eq!(double(&ctx, real_part), 3.0);
    let imaginary_part = call(&runtime, &mut ctx, "IMAGPART", &[conjugate]).unwrap();
    assert_eq!(double(&ctx, imaginary_part), -4.0);

    assert!(ncl_object::pop_root(&mut ctx, conjugate_root));
    assert!(ncl_object::pop_root(&mut ctx, complex_root));
    assert!(ncl_object::pop_root(&mut ctx, imaginary_root));
    assert!(ncl_object::pop_root(&mut ctx, real_root));
}

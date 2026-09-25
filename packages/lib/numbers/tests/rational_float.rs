#![allow(clippy::unwrap_used, missing_docs)]

use ncl_object::{
    classify_object, double_value, make_double, make_ratio, ratio_denominator, ratio_numerator,
    FunctionObject, ObjectRef, Runtime, ThreadContext, Word,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

#[test]
fn rational_accessors_and_conversion_preserve_exact_values() {
    let (runtime, mut ctx) = setup();
    let numerator = Word::fixnum(6);
    let denominator = Word::fixnum(8);
    let ratio = make_ratio(&mut ctx, &runtime, numerator, denominator).unwrap();
    let ratio_word = ratio.into();

    assert_eq!(call(&runtime, &mut ctx, "NUMERATOR", &[ratio_word]), numerator);
    assert_eq!(call(&runtime, &mut ctx, "DENOMINATOR", &[ratio_word]), denominator);
    let half: Word = make_double(&mut ctx, &runtime, 0.5).unwrap().into();
    let rational = call(&runtime, &mut ctx, "RATIONAL", &[half]);
    assert_eq!(classify_object(&ctx, rational), ObjectRef::Fixnum(1));
}

#[test]
fn rationalize_and_float_operations_return_expected_multiple_values() {
    let (runtime, mut ctx) = setup();
    let tenth = make_double(&mut ctx, &runtime, 0.1).unwrap().into();
    let rationalized = call(&runtime, &mut ctx, "RATIONALIZE", &[tenth]);
    assert!(matches!(classify_object(&ctx, rationalized), ObjectRef::Ratio(_)));
    let ratio = ncl_object::Ratio::from_word(rationalized);
    assert_eq!(ratio_numerator(&ctx, ratio).unwrap(), Word::fixnum(1));
    assert_eq!(ratio_denominator(&ctx, ratio).unwrap(), Word::fixnum(10));

    let value = make_double(&mut ctx, &runtime, 1.5).unwrap().into();
    let decoded = call(&runtime, &mut ctx, "DECODE-FLOAT", &[value]);
    assert_eq!(ctx.values().len(), 3);
    assert_eq!(double_value(&ctx, ncl_object::DoubleFloat::from_word(decoded)).unwrap(), 0.75);
    assert_eq!(ctx.values()[1], Word::fixnum(1));

    let scaled = call(&runtime, &mut ctx, "SCALE-FLOAT", &[value, Word::fixnum(2)]);
    assert_eq!(double_value(&ctx, ncl_object::DoubleFloat::from_word(scaled)).unwrap(), 6.0);
}

#[test]
fn float_metadata_matches_binary64() {
    let (runtime, mut ctx) = setup();
    let value = make_double(&mut ctx, &runtime, 1.0).unwrap().into();
    assert_eq!(call(&runtime, &mut ctx, "FLOAT-DIGITS", &[value]), Word::fixnum(53));
    assert_eq!(call(&runtime, &mut ctx, "FLOAT-PRECISION", &[value]), Word::fixnum(53));
    assert_eq!(call(&runtime, &mut ctx, "FLOAT-RADIX", &[value]), Word::fixnum(2));
    let sign = call(&runtime, &mut ctx, "FLOAT-SIGN", &[value]);
    assert_eq!(double_value(&ctx, ncl_object::DoubleFloat::from_word(sign)).unwrap(), 1.0);
}

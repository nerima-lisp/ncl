#![allow(clippy::unwrap_used, missing_docs, reason = "numeric builtin assertions")]

use ncl_object::{
    classify_object, double_value, symbol_value, FunctionObject, ObjectRef, Package, Runtime,
    ThreadContext, Word,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap())
        .unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn symbol(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

#[test]
fn registers_numeric_constants_with_expected_shapes() {
    let (runtime, mut ctx) = setup();
    let pi_symbol = symbol(&runtime, &mut ctx, "PI");
    let pi = symbol_value(&ctx, pi_symbol).unwrap();
    assert!(matches!(classify_object(&ctx, pi), ObjectRef::DoubleFloat(_)));
    let value = double_value(&ctx, ncl_object::DoubleFloat::from_word(pi)).unwrap();
    assert!((value - std::f64::consts::PI).abs() < f64::EPSILON);

    let max_symbol = symbol(&runtime, &mut ctx, "MOST-POSITIVE-FIXNUM");
    let max = symbol_value(&ctx, max_symbol).unwrap();
    assert_eq!(max.as_fixnum(), Some(i64::MAX >> 4));
    for name in [
        "SHORT-FLOAT-EPSILON",
        "SINGLE-FLOAT-EPSILON",
        "DOUBLE-FLOAT-EPSILON",
        "LONG-FLOAT-EPSILON",
        "LEAST-POSITIVE-DOUBLE-FLOAT",
        "MOST-NEGATIVE-DOUBLE-FLOAT",
    ] {
        let constant = symbol(&runtime, &mut ctx, name);
        assert!(symbol_value(&ctx, constant).is_ok(), "{name}");
    }
}

#[test]
fn random_state_is_reproducible_and_random_respects_integer_limit() {
    let (runtime, mut ctx) = setup();
    let state = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[Word::NIL]);
    assert_eq!(call(&runtime, &mut ctx, "RANDOM-STATE-P", &[state]), Word::TRUE);
    let copy = call(&runtime, &mut ctx, "MAKE-RANDOM-STATE", &[state]);
    let first = call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(100), state]);
    let second = call(&runtime, &mut ctx, "RANDOM", &[Word::fixnum(100), copy]);
    assert_eq!(first, second);
    assert!(first.as_fixnum().unwrap() >= 0);
    assert!(first.as_fixnum().unwrap() < 100);
}

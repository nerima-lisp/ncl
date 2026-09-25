#![allow(clippy::unwrap_used, missing_docs)]

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, classify_object};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime.function(ctx, "COMMON-LISP", name).unwrap();
    let function = ncl_object::FunctionObject::try_from(function).unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn integer(ctx: &ThreadContext, word: Word) -> i128 {
    match classify_object(ctx, word) {
        ObjectRef::Fixnum(value) => i128::from(value),
        _ => panic!("expected fixnum"),
    }
}

#[test]
fn exact_rounding_returns_the_quotient_and_remainder() {
    let (runtime, mut ctx) = setup();
    let value = Word::fixnum(-7);
    let divisor = Word::fixnum(2);

    let result = call(&runtime, &mut ctx, "FLOOR", &[value, divisor]);
    assert_eq!(integer(&ctx, result), -4);
    assert_eq!(integer(&ctx, ctx.values()[1]), 1);
    let result = call(&runtime, &mut ctx, "CEILING", &[value, divisor]);
    assert_eq!(integer(&ctx, result), -3);
    assert_eq!(integer(&ctx, ctx.values()[1]), -1);
    let result = call(&runtime, &mut ctx, "TRUNCATE", &[value, divisor]);
    assert_eq!(integer(&ctx, result), -3);
    assert_eq!(integer(&ctx, ctx.values()[1]), -1);
    let result = call(&runtime, &mut ctx, "ROUND", &[Word::fixnum(5), divisor]);
    assert_eq!(integer(&ctx, result), 2);
    assert_eq!(integer(&ctx, ctx.values()[1]), 1);
    let result = call(&runtime, &mut ctx, "ROUND", &[Word::fixnum(7), divisor]);
    assert_eq!(integer(&ctx, result), 4);
    assert_eq!(integer(&ctx, ctx.values()[1]), -1);
}

#[test]
fn float_rounding_variants_return_float_quotients() {
    let (runtime, mut ctx) = setup();
    let value = ncl_object::make_double(&mut ctx, &runtime, -7.5)
        .unwrap()
        .into();
    let quotient = call(&runtime, &mut ctx, "FFLOOR", &[value]);
    assert!(matches!(
        classify_object(&ctx, quotient),
        ObjectRef::DoubleFloat(_)
    ));
    assert!(matches!(
        classify_object(&ctx, ctx.values()[1]),
        ObjectRef::DoubleFloat(_)
    ));
}

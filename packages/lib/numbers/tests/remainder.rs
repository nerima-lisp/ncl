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
fn mod_and_rem_follow_common_lisp_sign_rules() {
    let (runtime, mut ctx) = setup();
    let value = Word::fixnum(-7);
    let divisor = Word::fixnum(2);

    let result = call(&runtime, &mut ctx, "MOD", &[value, divisor]);
    assert_eq!(integer(&ctx, result), 1);
    let result = call(&runtime, &mut ctx, "REM", &[value, divisor]);
    assert_eq!(integer(&ctx, result), -1);
    let result = call(
        &runtime,
        &mut ctx,
        "MOD",
        &[Word::fixnum(7), Word::fixnum(-2)],
    );
    assert_eq!(integer(&ctx, result), -1);
}

#[test]
fn integer_remainder_group_has_identity_and_exact_isqrt() {
    let (runtime, mut ctx) = setup();
    let result = call(&runtime, &mut ctx, "GCD", &[]);
    assert_eq!(integer(&ctx, result), 0);
    let result = call(
        &runtime,
        &mut ctx,
        "GCD",
        &[Word::fixnum(-18), Word::fixnum(24)],
    );
    assert_eq!(integer(&ctx, result), 6);
    let result = call(&runtime, &mut ctx, "LCM", &[]);
    assert_eq!(integer(&ctx, result), 1);
    let result = call(
        &runtime,
        &mut ctx,
        "LCM",
        &[Word::fixnum(-6), Word::fixnum(8)],
    );
    assert_eq!(integer(&ctx, result), 24);
    let result = call(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(15)]);
    assert_eq!(integer(&ctx, result), 3);
    let result = call(&runtime, &mut ctx, "ISQRT", &[Word::fixnum(16)]);
    assert_eq!(integer(&ctx, result), 4);
}

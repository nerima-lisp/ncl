#![allow(clippy::expect_used, clippy::unnecessary_mut_passed)]

use super::register;
use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word, make_simple_vector};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("thread context");
    crate::register(&runtime).expect("type registration");
    register(&runtime).expect("builtin registration");
    (runtime, ctx)
}

fn intern(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime.find_package(ctx, "COMMON-LISP").expect("package");
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .expect("symbol")
        .0
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", name)
            .expect("function"),
    )
    .expect("function object")
}

#[test]
fn builtins_cover_keywordp_subtypep_and_coerce_identity_paths() {
    let (runtime, mut ctx) = setup();
    let keywordp = function(&runtime, &mut ctx, "KEYWORDP");
    let subtypep = function(&runtime, &mut ctx, "SUBTYPEP");
    let coerce = function(&runtime, &mut ctx, "COERCE");
    let keyword = {
        let package = runtime
            .find_package(&mut ctx, "KEYWORD")
            .expect("keyword package");
        Package::from_word(package)
            .intern(&mut ctx, &runtime, "FLAG")
            .expect("keyword")
            .0
    };
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[keyword]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, keywordp, &[Word::NIL]),
        Ok(Word::NIL)
    );

    let invalid = Word::fixnum(8);
    assert_eq!(
        runtime.call_builtin(&mut ctx, subtypep, &[invalid, invalid]),
        Ok(Word::NIL)
    );
    let integer = intern(&mut ctx, &runtime, "INTEGER");
    assert_eq!(
        runtime.call_builtin(&mut ctx, coerce, &[Word::fixnum(3), integer]),
        Ok(Word::fixnum(3))
    );

    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).expect("vector");
    let vector_type = intern(&mut ctx, &runtime, "VECTOR");
    let converted = runtime
        .call_builtin(&mut ctx, coerce, &[vector, vector_type])
        .expect("vector coercion");
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, converted).expect("length"),
        1
    );
}

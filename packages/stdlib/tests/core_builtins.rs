#![allow(missing_docs)]

use ncl_object::{FunctionObject, ObjectError, Runtime, ThreadContext, Word};

fn registered_function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    let word = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("COMMON-LISP:{name} is not registered"));
    FunctionObject::try_from(word)
        .unwrap_or_else(|error| panic!("COMMON-LISP:{name} is not callable: {error:?}"))
}

#[test]
fn registered_car_and_cons_builtins_return_and_validate_values() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context registration: {error:?}"));
    ncl_stdlib::register_all(&mut ctx, &runtime)
        .unwrap_or_else(|error| panic!("stdlib registration: {error:?}"));

    let cons = registered_function(&runtime, &mut ctx, "CONS");
    let pair = runtime
        .call_builtin(&mut ctx, cons, &[Word::fixnum(7), Word::fixnum(8)])
        .unwrap_or_else(|error| panic!("CONS call: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, pair), Ok(Word::fixnum(7)));
    assert_eq!(ncl_object::cdr(&ctx, pair), Ok(Word::fixnum(8)));

    let car = registered_function(&runtime, &mut ctx, "CAR");
    assert_eq!(
        runtime.call_builtin(&mut ctx, car, &[pair]),
        Ok(Word::fixnum(7))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, car, &[Word::fixnum(7)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, cons, &[Word::fixnum(7)]),
        Err(ObjectError::TypeError)
    );
}

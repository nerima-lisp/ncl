//! Hash-table builtin integration tests.

use ncl_object::{FunctionObject, Runtime, ThreadContext, Word};

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    runtime
        .call_builtin(ctx, function, args)
        .unwrap_or_else(|error| panic!("{name}: {error:?}"))
}

#[test]
fn hash_builtins_cover_lifecycle_and_multiple_values() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("context: {error:?}"));
    ncl_lib_hash_arrays::register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));

    let table = call(&runtime, &mut ctx, "MAKE-HASH-TABLE", &[]);
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-P", &[table]),
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table]),
        Word::fixnum(0)
    );

    let key = Word::fixnum(7);
    let value = Word::fixnum(42);
    call(&runtime, &mut ctx, "GETHASH", &[key, table, value]);
    assert_eq!(ctx.values(), &[value, Word::NIL]);

    call(&runtime, &mut ctx, "GETHASH", &[key, table]);
    assert_eq!(ctx.values(), &[Word::NIL, Word::NIL]);
    call(&runtime, &mut ctx, "REMHASH", &[key, table]);
    assert_eq!(
        call(&runtime, &mut ctx, "HASH-TABLE-COUNT", &[table]),
        Word::fixnum(0)
    );

    call(&runtime, &mut ctx, "CLRHASH", &[table]);
    assert!(
        call(&runtime, &mut ctx, "SXHASH", &[key])
            .as_fixnum()
            .is_some()
    );
}

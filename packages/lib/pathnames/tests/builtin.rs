#![allow(missing_docs, clippy::expect_used)]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, classify_object, make_string, symbol_function,
};

fn common_lisp_function(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> FunctionObject {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .expect("COMMON-LISP");
    let (symbol, _) = Package::from_word(package)
        .intern(ctx, runtime, name)
        .expect("intern builtin");
    FunctionObject::try_from(symbol_function(ctx, symbol).expect("function cell"))
        .expect("function object")
}

#[test]
fn pathname_builtins_round_trip_through_lisp_heap() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("thread");
    ncl_lib_pathnames::register(&runtime).expect("pathname registration");

    let source = make_string(
        &mut ctx,
        &runtime,
        &"/tmp/report.txt".chars().collect::<Vec<_>>(),
    )
    .expect("source string");
    let pathname_function = common_lisp_function(&mut ctx, &runtime, "PATHNAME");
    let pathname = runtime
        .call_builtin(&mut ctx, pathname_function, &[source])
        .expect("pathname call");
    assert!(matches!(
        classify_object(&ctx, pathname),
        ncl_object::ObjectRef::Instance(_)
    ));

    let namestring_function = common_lisp_function(&mut ctx, &runtime, "NAMESTRING");
    let namestring = runtime
        .call_builtin(&mut ctx, namestring_function, &[pathname])
        .expect("namestring call");
    assert_eq!(
        ncl_object::string_length(&ctx, namestring).expect("length"),
        15
    );
}

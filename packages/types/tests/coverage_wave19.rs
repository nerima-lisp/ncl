#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "tests assert on public type behavior"
)]

use ncl_object::{Package, Runtime, ThreadContext, Word, make_bignum_from_i128, make_cons};
use ncl_types::{TypeSpecifier, Value, parse_type_specifier, typep};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

#[test]
fn parsed_opaque_values_keep_identity_for_eql_queries() {
    let (runtime, mut ctx) = setup();
    let first: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 40)
        .unwrap()
        .into();
    let second: Word = make_bignum_from_i128(&mut ctx, &runtime, 1_i128 << 40)
        .unwrap()
        .into();

    let package = runtime.find_package(&ctx, "COMMON-LISP").unwrap();
    let eql = Package::from_word(package)
        .intern(&mut ctx, &runtime, "EQL")
        .unwrap()
        .0;
    let argument = make_cons(&mut ctx, &runtime, first, Word::NIL).unwrap();
    let form = make_cons(&mut ctx, &runtime, eql, argument).unwrap();
    let specifier = parse_type_specifier(&mut ctx, form).unwrap();
    assert_eq!(specifier, TypeSpecifier::Eql(Value::Opaque(first.bits())));
    assert!(typep(&mut ctx, first, &specifier).unwrap());
    assert!(!typep(&mut ctx, second, &specifier).unwrap());
}

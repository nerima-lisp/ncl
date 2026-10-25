use super::*;

#[test]
fn register_sb_symbol_propagates_malformed_source_package_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let source = runtime
        .ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())
        .unwrap_or_else(|error| panic!("NCL-TEST package: {error:?}"));
    crate::object_access::put(&mut ctx, source, crate::package::INTERNAL, crate::Word::NIL)
        .unwrap_or_else(|error| panic!("corrupt source package fixture: {error:?}"));

    assert_eq!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-MALFORMED-SOURCE"),
            false,
        ),
        Err(ObjectError::TypeError)
    );
}

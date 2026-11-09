use super::*;
use crate::Word;

fn assert_malformed_source_package_is_rejected(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    source: Word,
) {
    crate::object_access::put(
        ctx,
        source,
        crate::package::INTERNAL,
        crate::Word::fixnum(0),
    )
    .unwrap_or_else(|error| panic!("corrupt source package fixture: {error:?}"));
    runtime
        .release_package_symbols(source)
        .unwrap_or_else(|error| panic!("release package symbol index: {error:?}"));
    assert_eq!(
        runtime.register_sb_symbol(
            ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-MALFORMED-SOURCE"),
            false,
        ),
        Err(ObjectError::Layout)
    );
}

#[test]
fn register_sb_symbol_propagates_malformed_source_package_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let source = runtime
        .ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())
        .unwrap_or_else(|error| panic!("NCL-TEST package: {error:?}"));

    assert_malformed_source_package_is_rejected(&runtime, &mut ctx, source);
}

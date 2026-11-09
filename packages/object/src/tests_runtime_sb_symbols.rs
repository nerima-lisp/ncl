use super::*;

#[test]
fn register_sb_symbol_propagates_malformed_source_package_error() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let source = runtime.ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())?;
    let absent_symbol = runtime.register_sb_symbol(
        &mut ctx,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-ABSENT-SOURCE-SYMBOL"),
        false,
    );
    crate::object_access::put(
        &mut ctx,
        source,
        crate::package::INTERNAL,
        crate::Word::fixnum(0),
    )?;
    let release_result = runtime.release_package_symbols(source);
    let malformed_source = runtime.register_sb_symbol(
        &mut ctx,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-MALFORMED-SOURCE"),
        false,
    );

    assert_eq!(
        (absent_symbol, release_result, malformed_source),
        (
            Err(ObjectError::Unsupported),
            Ok(()),
            Err(ObjectError::Layout)
        )
    );
    Ok(())
}

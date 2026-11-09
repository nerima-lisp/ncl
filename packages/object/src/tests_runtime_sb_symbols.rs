use super::*;
use crate::Word;

fn assert_malformed_source_package_is_rejected(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    source: Word,
) -> Result<(), ObjectError> {
    crate::object_access::put(
        ctx,
        source,
        crate::package::INTERNAL,
        crate::Word::fixnum(0),
    )?;
    runtime.release_package_symbols(source)?;
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
    Ok(())
}

#[test]
fn register_sb_symbol_propagates_malformed_source_package_error() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let source = runtime.ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())?;
    let target = runtime.ensure_package(&mut ctx, BuiltinPackage::SbDebug.as_str())?;

    let (source_symbol, _) =
        Package::from_word(source).intern(&mut ctx, &runtime, "R6-SB-IMPORT")?;
    let source_name = crate::symbol_name(&ctx, source_symbol)?;

    runtime.register_sb_symbol(
        &mut ctx,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-SB-IMPORT"),
        false,
    )?;
    assert_eq!(
        Package::from_word(target).find_symbol(&mut ctx, &runtime, source_name)?,
        Some((source_symbol, crate::FindStatus::Internal))
    );

    runtime.register_sb_symbol(
        &mut ctx,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-SB-IMPORT"),
        false,
    )?;
    runtime.register_sb_symbol(
        &mut ctx,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-SB-IMPORT"),
        true,
    )?;
    assert_eq!(
        Package::from_word(target).find_symbol(&mut ctx, &runtime, source_name)?,
        Some((source_symbol, crate::FindStatus::External))
    );
    runtime.register_sb_symbol(
        &mut ctx,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-SB-IMPORT"),
        true,
    )?;
    assert_eq!(crate::symbol_package(&ctx, source_symbol)?, source);

    let (homeless_symbol, _) =
        Package::from_word(source).intern(&mut ctx, &runtime, "R6-HOMELESS")?;
    crate::put(
        &mut ctx,
        homeless_symbol,
        crate::layout::symbol_offset::PACKAGE,
        Word::NIL,
    )?;
    runtime.register_sb_symbol(
        &mut ctx,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-HOMELESS"),
        true,
    )?;
    assert_eq!(crate::symbol_package(&ctx, homeless_symbol)?, target);

    assert_malformed_source_package_is_rejected(&runtime, &mut ctx, source)?;
    Ok(())
}

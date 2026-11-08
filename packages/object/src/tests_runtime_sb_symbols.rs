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

    assert_eq!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::CommonLisp,
            BuiltinPackage::SbDebug,
            BuiltinName::new("CAR"),
            false,
        ),
        Err(ObjectError::Unsupported)
    );
    assert_eq!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::NclTest,
            BuiltinName::new("R6-INVALID-TARGET"),
            false,
        ),
        Err(ObjectError::Unsupported)
    );
    assert_eq!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-MISSING-SOURCE"),
            false,
        ),
        Err(ObjectError::Unsupported)
    );

    let _ = Package::from_word(source)
        .intern(&mut ctx, &runtime, "R6-REGISTERED")
        .unwrap_or_else(|error| panic!("intern registered source symbol: {error:?}"));
    for external in [false, false, true, true] {
        assert_eq!(
            runtime.register_sb_symbol(
                &mut ctx,
                BuiltinPackage::NclTest,
                BuiltinPackage::SbDebug,
                BuiltinName::new("R6-REGISTERED"),
                external,
            ),
            Ok(())
        );
    }

    let (conflict_source, _) = Package::from_word(source)
        .intern(&mut ctx, &runtime, "R6-CONFLICT")
        .unwrap_or_else(|error| panic!("intern conflict source symbol: {error:?}"));
    let target = runtime
        .ensure_package(&mut ctx, BuiltinPackage::SbDebug.as_str())
        .unwrap_or_else(|error| panic!("SB-DEBUG package: {error:?}"));
    let (conflict_target, _) = Package::from_word(target)
        .intern(&mut ctx, &runtime, "R6-CONFLICT")
        .unwrap_or_else(|error| panic!("intern conflict target symbol: {error:?}"));
    assert_ne!(conflict_source, conflict_target);
    assert_eq!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-CONFLICT"),
            false,
        ),
        Err(ObjectError::PackageConflict)
    );

    let (homeless, _) = Package::from_word(source)
        .intern(&mut ctx, &runtime, "R6-HOMELESS")
        .unwrap_or_else(|error| panic!("intern homeless symbol: {error:?}"));
    crate::object_access::put(
        &mut ctx,
        homeless,
        crate::layout::symbol_offset::PACKAGE,
        crate::Word::NIL,
    )
    .unwrap_or_else(|error| panic!("clear symbol home package: {error:?}"));
    assert_eq!(
        runtime.register_sb_symbol(
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-HOMELESS"),
            true,
        ),
        Ok(())
    );

    crate::object_access::put(
        &mut ctx,
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
            &mut ctx,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-MALFORMED-SOURCE"),
            false,
        ),
        Err(ObjectError::Layout)
    );
}

use crate::{register_gc_sb_ext_symbols, runtime_sb_symbols::register_sb_symbol};
use ncl_object::package::FindStatus;
use ncl_object::{
    BuiltinName, BuiltinPackage, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_string, pop_root, push_root, symbol_package,
};
use ncl_packages::{PackageListOperations, PackageOperations};

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string allocation: {error:?}"))
}

fn assert_homeless_sb_symbol_can_be_registered(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    source: Word,
) {
    let (mut homeless, _) = Package::from_word(source)
        .intern(ctx, runtime, "R6-HOMELESS")
        .unwrap_or_else(|error| panic!("intern homeless symbol: {error:?}"));
    let homeless_token = push_root(ctx, &mut homeless);
    ncl_object::put(
        ctx,
        homeless,
        ncl_object::layout::symbol_offset::PACKAGE,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("clear symbol home package: {error:?}"));
    assert_eq!(
        register_sb_symbol(
            ctx,
            runtime,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-HOMELESS"),
            true,
        ),
        Ok(())
    );
    let target = runtime
        .find_package(ctx, BuiltinPackage::SbDebug.as_str())
        .unwrap_or_else(|| panic!("SB-DEBUG package missing"));
    assert_eq!(
        symbol_package(ctx, homeless)
            .unwrap_or_else(|error| panic!("homeless symbol home: {error:?}")),
        target
    );
    assert!(pop_root(ctx, homeless_token));
}

fn assert_inherited_sb_symbol_is_imported(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    mut source: Word,
    mut target: Word,
) {
    let source_token = push_root(ctx, &mut source);
    let target_token = push_root(ctx, &mut target);
    let mut name = string(ctx, runtime, "R6-INHERITED-SYMBOL");
    let name_token = push_root(ctx, &mut name);
    let (mut symbol, _) = Package::from_word(source)
        .intern(ctx, runtime, "R6-INHERITED-SYMBOL")
        .unwrap_or_else(|error| panic!("source intern: {error:?}"));
    let symbol_token = push_root(ctx, &mut symbol);

    assert_eq!(
        Package::from_word(source).export(ctx, runtime, name),
        Ok(true)
    );
    assert_eq!(
        Package::from_word(target).use_package(ctx, runtime, source),
        Ok(true)
    );
    assert_eq!(
        Package::from_word(target).find_symbol(ctx, runtime, name),
        Ok(Some((symbol, FindStatus::Inherited)))
    );
    register_sb_symbol(
        ctx,
        runtime,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-INHERITED-SYMBOL"),
        false,
    )
    .unwrap_or_else(|error| panic!("inherited SB import: {error:?}"));
    assert_eq!(
        Package::from_word(target).find_symbol(ctx, runtime, name),
        Ok(Some((symbol, FindStatus::Internal)))
    );
    assert_eq!(symbol_package(ctx, symbol), Ok(source));

    assert!(pop_root(ctx, symbol_token));
    assert!(pop_root(ctx, name_token));
    assert!(pop_root(ctx, target_token));
    assert!(pop_root(ctx, source_token));
}

const OBJECT_OWNERSHIP: &str = include_str!("../../../../object/ownership.tsv");
const LIB_PACKAGES_OWNERSHIP: &str = include_str!("../../ownership.tsv");
const SB_EXT_SURFACE: &str = include_str!("../../../../../conformance/sbcl/symbols/sb-ext.tsv");

struct OwnershipRow {
    package: String,
    symbol: String,
}

fn ownership_rows(table: &str, crate_name: &str) -> Vec<OwnershipRow> {
    table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 7);
            (fields[3] == crate_name && fields[4] == "1").then(|| OwnershipRow {
                package: fields[0].to_owned(),
                symbol: fields[1].to_owned(),
            })
        })
        .collect()
}

#[test]
fn package_adapter_sb_ext_symbols_share_ncl_identity_and_surface_visibility() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ncl_object::register(&mut ctx, &runtime)
        .unwrap_or_else(|error| panic!("NCL-GC register: {error:?}"));
    register_gc_sb_ext_symbols(&mut ctx, &runtime)
        .unwrap_or_else(|error| panic!("SB-EXT symbol registration: {error:?}"));

    let object_ownership = ownership_rows(OBJECT_OWNERSHIP, "ncl-object");
    let package_ownership = ownership_rows(LIB_PACKAGES_OWNERSHIP, "ncl-lib-packages");
    let sb_ext_rows: Vec<_> = package_ownership
        .iter()
        .filter(|row| row.package == "SB-EXT")
        .collect();
    assert_eq!(sb_ext_rows.len(), 14);

    for row in sb_ext_rows {
        let source_rows: Vec<_> = object_ownership
            .iter()
            .filter(|source| source.package.starts_with("NCL-") && source.symbol == row.symbol)
            .collect();
        assert_eq!(source_rows.len(), 1, "{} ownership source", row.symbol);
        let source_package_name = &source_rows[0].package;
        let mut source_package = runtime
            .find_package(&ctx, source_package_name)
            .unwrap_or_else(|| panic!("{source_package_name} package missing"));
        let source_package_token = ncl_object::push_root(&mut ctx, &mut source_package);
        let mut sb_package = runtime
            .find_package(&ctx, "SB-EXT")
            .unwrap_or_else(|| panic!("SB-EXT package missing"));
        let sb_package_token = ncl_object::push_root(&mut ctx, &mut sb_package);
        let mut name = string(&mut ctx, &runtime, &row.symbol);
        let name_token = ncl_object::push_root(&mut ctx, &mut name);

        let (source_symbol, _) = Package::from_word(source_package)
            .find_symbol(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("{} source lookup: {error:?}", row.symbol))
            .unwrap_or_else(|| panic!("{} source symbol missing", row.symbol));
        let mut source_symbol = source_symbol;
        let source_symbol_token = ncl_object::push_root(&mut ctx, &mut source_symbol);
        let home_before = symbol_package(&ctx, source_symbol)
            .unwrap_or_else(|error| panic!("{} home before import: {error:?}", row.symbol));
        assert_eq!(home_before, source_package, "{} source home", row.symbol);

        let (sb_symbol, sb_status) = Package::from_word(sb_package)
            .find_symbol(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("{} SB lookup: {error:?}", row.symbol))
            .unwrap_or_else(|| panic!("{} SB symbol missing", row.symbol));
        assert_eq!(sb_symbol, source_symbol, "{} symbol identity", row.symbol);
        let surface_row = SB_EXT_SURFACE
            .lines()
            .skip(1)
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .find(|fields| fields.len() >= 4 && fields[0] == "SB-EXT" && fields[1] == row.symbol)
            .unwrap_or_else(|| panic!("{} SBCL surface row missing", row.symbol));
        assert_eq!(surface_row[2], "yes", "{} SBCL presence", row.symbol);
        let expected_status = match surface_row[3] {
            "yes" => FindStatus::External,
            "no" => FindStatus::Internal,
            external => panic!("{} invalid SBCL external flag {external}", row.symbol),
        };
        assert_eq!(sb_status, expected_status, "{} external status", row.symbol);
        let home_after = symbol_package(&ctx, sb_symbol)
            .unwrap_or_else(|error| panic!("{} home after import: {error:?}", row.symbol));
        assert_eq!(home_after, home_before, "{} home package", row.symbol);

        assert!(ncl_object::pop_root(&mut ctx, source_symbol_token));
        assert!(ncl_object::pop_root(&mut ctx, name_token));
        assert!(ncl_object::pop_root(&mut ctx, sb_package_token));
        assert!(ncl_object::pop_root(&mut ctx, source_package_token));
    }
}

#[test]
fn register_sb_symbol_honors_external_flag_and_rejects_invalid_inputs() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));

    let mut source = runtime
        .ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())
        .unwrap_or_else(|error| panic!("NCL-TEST package: {error:?}"));
    let source_token = push_root(&mut ctx, &mut source);
    let mut name = string(&mut ctx, &runtime, "R6-PRIVATE-SYMBOL");
    let name_token = push_root(&mut ctx, &mut name);
    let (mut source_symbol, _) = Package::from_word(source)
        .intern(&mut ctx, &runtime, "R6-PRIVATE-SYMBOL")
        .unwrap_or_else(|error| panic!("source intern: {error:?}"));
    let symbol_token = push_root(&mut ctx, &mut source_symbol);

    register_sb_symbol(
        &mut ctx,
        &runtime,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-PRIVATE-SYMBOL"),
        false,
    )
    .unwrap_or_else(|error| panic!("SB import: {error:?}"));
    let mut target = runtime
        .find_package(&ctx, BuiltinPackage::SbDebug.as_str())
        .unwrap_or_else(|| panic!("SB-DEBUG package missing"));
    let target_token = push_root(&mut ctx, &mut target);
    assert_eq!(
        Package::from_word(target).find_symbol(&mut ctx, &runtime, name),
        Ok(Some((source_symbol, FindStatus::Internal)))
    );
    for external in [false, false, true, true] {
        assert_eq!(
            register_sb_symbol(
                &mut ctx,
                &runtime,
                BuiltinPackage::NclTest,
                BuiltinPackage::SbDebug,
                BuiltinName::new("R6-PRIVATE-SYMBOL"),
                external,
            ),
            Ok(())
        );
    }
    assert_eq!(
        Package::from_word(target).find_symbol(&mut ctx, &runtime, name),
        Ok(Some((source_symbol, FindStatus::External)))
    );
    assert_eq!(symbol_package(&ctx, source_symbol), Ok(source));

    assert_homeless_sb_symbol_can_be_registered(&runtime, &mut ctx, source);

    assert_inherited_sb_symbol_is_imported(&runtime, &mut ctx, source, target);

    for (source_package, target_package, symbol_name) in [
        (
            BuiltinPackage::CommonLisp,
            BuiltinPackage::SbDebug,
            "R6-PRIVATE-SYMBOL",
        ),
        (
            BuiltinPackage::NclTest,
            BuiltinPackage::NclFfi,
            "R6-PRIVATE-SYMBOL",
        ),
        (
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            "R6-ABSENT-SYMBOL",
        ),
        (BuiltinPackage::CommonLisp, BuiltinPackage::SbDebug, "CAR"),
        (
            BuiltinPackage::NclTest,
            BuiltinPackage::NclTest,
            "R6-INVALID-TARGET",
        ),
        (
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            "R6-MISSING-SOURCE",
        ),
    ] {
        assert_eq!(
            register_sb_symbol(
                &mut ctx,
                &runtime,
                source_package,
                target_package,
                BuiltinName::new(symbol_name),
                false,
            ),
            Err(ObjectError::Unsupported)
        );
    }

    assert!(pop_root(&mut ctx, target_token));
    assert!(pop_root(&mut ctx, symbol_token));
    assert!(pop_root(&mut ctx, name_token));
    assert!(pop_root(&mut ctx, source_token));
}

#[test]
fn register_sb_symbol_rejects_distinct_target_symbol() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let source = runtime
        .ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())
        .unwrap_or_else(|error| panic!("NCL-TEST package: {error:?}"));
    let target = runtime
        .ensure_package(&mut ctx, BuiltinPackage::SbDebug.as_str())
        .unwrap_or_else(|error| panic!("SB-DEBUG package: {error:?}"));
    let (source_symbol, _) = Package::from_word(source)
        .intern(&mut ctx, &runtime, "R6-CONFLICTING-SYMBOL")
        .unwrap_or_else(|error| panic!("NCL-TEST intern: {error:?}"));
    let (target_symbol, _) = Package::from_word(target)
        .intern(&mut ctx, &runtime, "R6-CONFLICTING-SYMBOL")
        .unwrap_or_else(|error| panic!("SB-DEBUG intern: {error:?}"));
    assert_ne!(source_symbol, target_symbol);

    assert_eq!(
        register_sb_symbol(
            &mut ctx,
            &runtime,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-CONFLICTING-SYMBOL"),
            false,
        ),
        Err(ObjectError::PackageConflict)
    );
}

#[test]
fn register_sb_symbol_propagates_unregistered_context_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();

    assert_eq!(
        register_sb_symbol(
            &mut ctx,
            &runtime,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new("R6-UNREGISTERED-CONTEXT"),
            false,
        ),
        Err(ObjectError::TypeError)
    );
}

#[test]
fn register_sb_symbol_propagates_oversized_name_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let name = Box::leak("A".repeat(8_500_000).into_boxed_str());

    assert!(matches!(
        register_sb_symbol(
            &mut ctx,
            &runtime,
            BuiltinPackage::NclTest,
            BuiltinPackage::SbDebug,
            BuiltinName::new(name),
            false,
        ),
        Err(ObjectError::Storage(_))
    ));
}

#[test]
fn gc_registration_rejects_conflicting_sb_ext_symbol() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let sb_ext = runtime
        .find_package(&ctx, BuiltinPackage::SbExt.as_str())
        .unwrap_or_else(|| panic!("SB-EXT package missing"));
    Package::from_word(sb_ext)
        .intern(&mut ctx, &runtime, "*AFTER-GC-HOOKS*")
        .unwrap_or_else(|error| panic!("SB-EXT intern: {error:?}"));

    ncl_object::register(&mut ctx, &runtime)
        .unwrap_or_else(|error| panic!("NCL-GC register: {error:?}"));
    assert_eq!(
        register_gc_sb_ext_symbols(&mut ctx, &runtime),
        Err(ObjectError::PackageConflict)
    );
}

#[test]
fn register_sb_symbol_propagates_malformed_source_package_error() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let source = runtime.ensure_package(&mut ctx, BuiltinPackage::NclTest.as_str())?;
    let absent_symbol = register_sb_symbol(
        &mut ctx,
        &runtime,
        BuiltinPackage::NclTest,
        BuiltinPackage::SbDebug,
        BuiltinName::new("R6-ABSENT-SOURCE-SYMBOL"),
        false,
    );
    ncl_object::put(
        &mut ctx,
        source,
        ncl_object::package::INTERNAL,
        Word::fixnum(0),
    )?;
    let release_result = runtime.release_package_symbols(source);
    let malformed_source = register_sb_symbol(
        &mut ctx,
        &runtime,
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

#[test]
fn gc_registers_a_callable_builtin() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ncl_object::register(&mut ctx, &runtime)
        .unwrap_or_else(|error| panic!("NCL-GC registration: {error:?}"));
    register_gc_sb_ext_symbols(&mut ctx, &runtime)
        .unwrap_or_else(|error| panic!("SB-EXT symbol registration: {error:?}"));
    let gc = runtime
        .function(&mut ctx, "NCL-GC", "GC")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("GC builtin missing"));
    let sb_gc = runtime
        .function(&mut ctx, "SB-EXT", "GC")
        .and_then(|word| FunctionObject::try_from(word).ok())
        .unwrap_or_else(|| panic!("SB-EXT GC function was not registered"));
    assert_eq!(sb_gc.as_word(), gc.as_word());

    assert_eq!(runtime.call_builtin(&mut ctx, gc, &[]), Ok(Word::NIL));
}

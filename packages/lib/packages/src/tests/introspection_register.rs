use super::*;
use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::string_offset;

// These slots corrupt test fixtures to exercise malformed object layouts.
const PACKAGE_USE_LIST_SLOT: usize = 4;
const PACKAGE_INTERNAL_SLOT: usize = 6;
const PACKAGE_EXTERNAL_SLOT: usize = 7;
const HASH_TABLE_TEST_SLOT: usize = 0;

fn registered_runtime() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

fn ensure_package(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Package, ObjectError> {
    Ok(Package::from_word(runtime.ensure_package(ctx, name)?))
}

fn table_with_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: Word,
    value: Word,
) -> Result<HashTable, ObjectError> {
    let table = HashTable::new(ctx, runtime, HashTest::Equal, Weakness::None)?;
    table.insert(ctx, runtime, key, value)?;
    Ok(table)
}

fn set_use_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: Package,
    used: Word,
) -> Result<(), ObjectError> {
    let use_list = ncl_object::make_cons(ctx, runtime, used, Word::NIL)?;
    ctx.write_object_slot(package.as_word(), PACKAGE_USE_LIST_SLOT, use_list)
}

#[test]
fn package_iteration_helper_errors_are_propagated() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = registered_runtime()?;
    assert_eq!(
        string_word_equals(&ctx, Word::fixnum(0), "X"),
        Err(ObjectError::TypeError)
    );

    let malformed_string = ncl_object::make_string(&mut ctx, &runtime, &['X'])?;
    ctx.write_object_slot(malformed_string, string_offset::DATA, Word::fixnum(0))?;
    assert_eq!(
        string_word_equals(&ctx, malformed_string, "X"),
        Err(ObjectError::Layout)
    );

    let mut unregistered = ThreadContext::new();
    assert!(symbol_list(&mut unregistered, &runtime, vec![Word::NIL]).is_err());
    assert!(all_package_iteration_symbols(&mut unregistered, &runtime).is_err());
    Ok(())
}

#[test]
fn package_iteration_reports_malformed_symbol_tables() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = registered_runtime()?;
    assert_eq!(
        package_iteration_symbols_for(
            &mut ctx,
            &runtime,
            Package::from_word(Word::fixnum(0)),
            false,
        ),
        Err(ObjectError::TypeError)
    );

    let malformed = ensure_package(&mut ctx, &runtime, "N25-ITERATION-BAD-TABLE")?;
    ctx.write_object_slot(malformed.as_word(), PACKAGE_INTERNAL_SLOT, Word::fixnum(0))?;
    assert!(package_iteration_symbols_for(&mut ctx, &runtime, malformed, false).is_err());
    assert_eq!(
        package_iteration_symbols_for(&mut ctx, &runtime, malformed, true),
        Err(ObjectError::TypeError)
    );
    assert!(all_package_iteration_symbols(&mut ctx, &runtime).is_err());

    let bad_symbol = ensure_package(&mut ctx, &runtime, "N25-ITERATION-BAD-SYMBOL")?;
    let key = ncl_object::make_string(&mut ctx, &runtime, &['B', 'A', 'D'])?;
    let table = table_with_entry(&mut ctx, &runtime, key, Word::fixnum(0))?;
    ctx.write_object_slot(bad_symbol.as_word(), PACKAGE_INTERNAL_SLOT, table.as_word())?;
    assert_eq!(
        package_iteration_symbols_for(&mut ctx, &runtime, bad_symbol, true),
        Err(ObjectError::TypeError)
    );

    let bad_lookup = ensure_package(&mut ctx, &runtime, "N25-ITERATION-BAD-LOOKUP")?;
    let (symbol, _) = bad_lookup.intern(&mut ctx, &runtime, "EXTERNAL")?;
    let name = ncl_object::symbol_name(&ctx, symbol)?;
    bad_lookup.export(&mut ctx, &runtime, name)?;
    let table = table_with_entry(&mut ctx, &runtime, name, symbol)?;
    ctx.write_object_slot(table.as_word(), HASH_TABLE_TEST_SLOT, Word::fixnum(99))?;
    ctx.write_object_slot(bad_lookup.as_word(), PACKAGE_EXTERNAL_SLOT, table.as_word())?;
    assert!(package_iteration_symbols_for(&mut ctx, &runtime, bad_lookup, true).is_err());
    Ok(())
}

#[test]
fn package_iteration_reports_malformed_use_and_inherited_tables() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = registered_runtime()?;

    let bad_use_list = ensure_package(&mut ctx, &runtime, "N25-ITERATION-BAD-USE-LIST")?;
    ctx.write_object_slot(
        bad_use_list.as_word(),
        PACKAGE_USE_LIST_SLOT,
        Word::fixnum(0),
    )?;
    assert!(package_iteration_symbols_for(&mut ctx, &runtime, bad_use_list, false).is_err());

    let bad_used_table = ensure_package(&mut ctx, &runtime, "N25-ITERATION-BAD-USED-TABLE")?;
    ctx.write_object_slot(
        bad_used_table.as_word(),
        PACKAGE_INTERNAL_SLOT,
        Word::fixnum(0),
    )?;
    let target = ensure_package(&mut ctx, &runtime, "N25-ITERATION-TARGET-USED-TABLE")?;
    set_use_list(&mut ctx, &runtime, target, bad_used_table.as_word())?;
    assert!(package_iteration_symbols_for(&mut ctx, &runtime, target, false).is_err());

    let bad_candidate = ensure_package(&mut ctx, &runtime, "N25-ITERATION-BAD-CANDIDATE")?;
    let key = ncl_object::make_string(&mut ctx, &runtime, &['B', 'A', 'D'])?;
    let table = table_with_entry(&mut ctx, &runtime, key, Word::fixnum(0))?;
    ctx.write_object_slot(
        bad_candidate.as_word(),
        PACKAGE_INTERNAL_SLOT,
        table.as_word(),
    )?;
    let target = ensure_package(&mut ctx, &runtime, "N25-ITERATION-TARGET-BAD-CANDIDATE")?;
    set_use_list(&mut ctx, &runtime, target, bad_candidate.as_word())?;
    assert_eq!(
        package_iteration_symbols_for(&mut ctx, &runtime, target, false),
        Err(ObjectError::TypeError)
    );

    let bad_used_lookup = ensure_package(&mut ctx, &runtime, "N25-ITERATION-BAD-USED-LOOKUP")?;
    let (symbol, _) = bad_used_lookup.intern(&mut ctx, &runtime, "EXTERNAL")?;
    let name = ncl_object::symbol_name(&ctx, symbol)?;
    bad_used_lookup.export(&mut ctx, &runtime, name)?;
    let table = table_with_entry(&mut ctx, &runtime, name, symbol)?;
    ctx.write_object_slot(table.as_word(), HASH_TABLE_TEST_SLOT, Word::fixnum(99))?;
    ctx.write_object_slot(
        bad_used_lookup.as_word(),
        PACKAGE_EXTERNAL_SLOT,
        table.as_word(),
    )?;
    let target = ensure_package(&mut ctx, &runtime, "N25-ITERATION-TARGET-BAD-USED-LOOKUP")?;
    set_use_list(&mut ctx, &runtime, target, bad_used_lookup.as_word())?;
    assert!(package_iteration_symbols_for(&mut ctx, &runtime, target, false).is_err());

    let bad_target_lookup = ensure_package(&mut ctx, &runtime, "N25-ITERATION-BAD-TARGET-LOOKUP")?;
    let table = HashTable::new(&mut ctx, &runtime, HashTest::Equal, Weakness::None)?;
    ctx.write_object_slot(table.as_word(), HASH_TABLE_TEST_SLOT, Word::fixnum(99))?;
    ctx.write_object_slot(
        bad_target_lookup.as_word(),
        PACKAGE_INTERNAL_SLOT,
        table.as_word(),
    )?;
    let used = ensure_package(&mut ctx, &runtime, "N25-ITERATION-TARGET-LOOKUP-USED")?;
    let (symbol, _) = used.intern(&mut ctx, &runtime, "EXTERNAL")?;
    let name = ncl_object::symbol_name(&ctx, symbol)?;
    used.export(&mut ctx, &runtime, name)?;
    set_use_list(&mut ctx, &runtime, bad_target_lookup, used.as_word())?;
    assert!(package_iteration_symbols_for(&mut ctx, &runtime, bad_target_lookup, false).is_err());
    Ok(())
}

#[test]
fn package_iteration_mode_comparisons_report_layout_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = registered_runtime()?;
    let keyword = Package::from_word(
        runtime
            .find_package(&ctx, "KEYWORD")
            .ok_or(ObjectError::Layout)?,
    );

    for name in ["ALL", "EXTERNAL", "SYMBOLS"] {
        let (mode, _) = keyword.intern(&mut ctx, &runtime, name)?;
        let string = ncl_object::symbol_name(&ctx, mode)?;
        ctx.write_object_slot(string, string_offset::DATA, Word::fixnum(0))?;
        let args = [mode, Word::NIL];
        assert_eq!(
            package_iteration_symbols(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&args),
                &mut MultipleValues::new(),
            ),
            Err(ObjectError::Layout)
        );
    }
    Ok(())
}

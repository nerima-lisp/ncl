//! Package symbol unintern behavior.

use ncl_hash_tables::HashTable;
use ncl_object::package::{EXTERNAL, INTERNAL, Package};
use ncl_object::{Runtime, ThreadContext, get, make_string, make_symbol, put, widetag};
use ncl_packages::PackageListOperations;

#[test]
fn unintern_does_not_remove_external_when_internal_matches() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = Package::new(&mut ctx, &runtime, "DUPLICATES")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let name = make_string(&mut ctx, &runtime, &['D', 'U', 'P'])
        .unwrap_or_else(|error| panic!("name: {error:?}"));
    let internal =
        make_symbol(&mut ctx, &runtime, name).unwrap_or_else(|error| panic!("internal: {error:?}"));
    let external =
        make_symbol(&mut ctx, &runtime, name).unwrap_or_else(|error| panic!("external: {error:?}"));
    put(
        &mut ctx,
        internal,
        ncl_object::layout::symbol_offset::PACKAGE,
        package.as_word(),
    )
    .unwrap_or_else(|error| panic!("internal home: {error:?}"));
    put(
        &mut ctx,
        external,
        ncl_object::layout::symbol_offset::PACKAGE,
        package.as_word(),
    )
    .unwrap_or_else(|error| panic!("external home: {error:?}"));
    let internal_table = HashTable::from_word(
        get(&ctx, package.as_word(), widetag::PACKAGE, INTERNAL)
            .unwrap_or_else(|error| panic!("internal table: {error:?}")),
    );
    internal_table
        .insert(&mut ctx, &runtime, name, internal)
        .unwrap_or_else(|error| panic!("internal insert: {error:?}"));
    let external_table = HashTable::from_word(
        get(&ctx, package.as_word(), widetag::PACKAGE, EXTERNAL)
            .unwrap_or_else(|error| panic!("external table: {error:?}")),
    );
    external_table
        .insert(&mut ctx, &runtime, name, external)
        .unwrap_or_else(|error| panic!("external insert: {error:?}"));

    assert!(
        package
            .unintern(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("unintern: {error:?}"))
    );
    assert_eq!(external_table.get(&mut ctx, name), Ok(Some(external)));
    assert_eq!(internal_table.get(&mut ctx, name), Ok(None));
}

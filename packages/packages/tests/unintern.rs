//! Package symbol unintern behavior.

use ncl_object::{Package, Runtime, ThreadContext, make_string, make_symbol, put};
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
    package
        .insert_table_symbol(&mut ctx, &runtime, false, name, internal)
        .unwrap_or_else(|error| panic!("internal insert: {error:?}"));
    package
        .insert_table_symbol(&mut ctx, &runtime, true, name, external)
        .unwrap_or_else(|error| panic!("external insert: {error:?}"));

    assert!(
        package
            .unintern(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("unintern: {error:?}"))
    );
    assert_eq!(
        package.table_symbol(&ctx, &runtime, true, name),
        Ok(Some(external))
    );
    assert_eq!(package.table_symbol(&ctx, &runtime, false, name), Ok(None));
}

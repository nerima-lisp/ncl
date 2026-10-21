#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "package tests assert on valid fixture construction"
)]

use super::super::Package;
use super::{Runtime, ThreadContext, Word};
use crate::FindStatus;

fn setup() -> (Runtime, ThreadContext, Package) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = Package::new(&mut ctx, &runtime, "LISTS-BEHAVIOR")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    (runtime, ctx, package)
}

fn list_members(ctx: &ThreadContext, mut list: Word) -> Vec<Word> {
    let mut members = Vec::new();
    while list != Word::NIL {
        members.push(ncl_sys::read_cons_word(&ctx.thread, list, 0).expect("list car"));
        list = ncl_sys::read_cons_word(&ctx.thread, list, 1).expect("list cdr");
    }
    members
}

fn assert_use_list(ctx: &ThreadContext, package: Package, expected: &[Word]) {
    let list = package.use_list(ctx).expect("use list");
    assert_eq!(list_members(ctx, list).as_slice(), expected);
}

fn assert_used_by_list(ctx: &ThreadContext, package: Package, expected: &[Word]) {
    let list = package.used_by_list(ctx).expect("used-by list");
    assert_eq!(list_members(ctx, list).as_slice(), expected);
}

fn assert_shadowing_list(ctx: &ThreadContext, package: Package, expected: &[Word]) {
    let list = package.shadowing_symbols(ctx).expect("shadow list");
    assert_eq!(list_members(ctx, list).as_slice(), expected);
}

#[test]
fn use_package_updates_both_lists_and_duplicate_removal_is_stable() {
    let (runtime, mut ctx, package) = setup();
    let source = Package::new(&mut ctx, &runtime, "SOURCE-LISTS-OK")
        .unwrap_or_else(|error| panic!("source: {error:?}"));
    assert_eq!(
        package.use_package(&mut ctx, &runtime, source.as_word()),
        Ok(true)
    );
    assert_eq!(
        package.use_package(&mut ctx, &runtime, source.as_word()),
        Ok(false)
    );
    assert_use_list(&ctx, package, &[source.as_word()]);
    assert_used_by_list(&ctx, source, &[package.as_word()]);
    assert_eq!(package.unuse_package(&mut ctx, source.as_word()), Ok(true));
    assert_eq!(package.unuse_package(&mut ctx, source.as_word()), Ok(false));
    assert_use_list(&ctx, package, &[]);
    assert_used_by_list(&ctx, source, &[]);
}

#[test]
fn shadow_and_shadowing_import_are_internal_idempotent_and_uninternable() {
    let (runtime, mut ctx, package) = setup();
    let shadow_name = crate::make_string(&mut ctx, &runtime, &['S', 'H']).expect("shadow name");
    package
        .shadow(&mut ctx, &runtime, shadow_name)
        .expect("shadow");
    let (shadow, status) = package
        .find_symbol(&mut ctx, shadow_name)
        .expect("find shadow")
        .expect("shadow symbol");
    assert_eq!(status, FindStatus::Internal);
    package
        .shadow(&mut ctx, &runtime, shadow_name)
        .expect("duplicate shadow");
    assert_shadowing_list(&ctx, package, &[shadow]);
    assert_eq!(package.unintern(&mut ctx, &runtime, shadow_name), Ok(true));
    assert_eq!(package.find_symbol(&mut ctx, shadow_name), Ok(None));
    assert_shadowing_list(&ctx, package, &[]);

    let source = Package::new(&mut ctx, &runtime, "IMPORT-SOURCE")
        .unwrap_or_else(|error| panic!("source: {error:?}"));
    let (symbol, source_status) = source
        .intern(&mut ctx, &runtime, "IM")
        .expect("intern import");
    assert_eq!(source_status, FindStatus::Internal);
    let name = crate::symbol_name(&ctx, symbol).expect("imported symbol name");
    package
        .shadowing_import(&mut ctx, &runtime, symbol)
        .expect("shadowing import");
    package
        .shadowing_import(&mut ctx, &runtime, symbol)
        .expect("duplicate import");
    assert_eq!(
        package.find_symbol(&mut ctx, name),
        Ok(Some((symbol, FindStatus::Internal)))
    );
    assert_shadowing_list(&ctx, package, &[symbol]);
    assert_eq!(package.unintern(&mut ctx, &runtime, name), Ok(true));
    assert_eq!(package.find_symbol(&mut ctx, name), Ok(None));
    assert_shadowing_list(&ctx, package, &[]);
    assert_eq!(
        source.find_symbol(&mut ctx, name),
        Ok(Some((symbol, FindStatus::Internal)))
    );
}

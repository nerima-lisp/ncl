//! Package name index and nickname lifecycle tests.

use ncl_object::package::Package;
use ncl_object::{ObjectError, Runtime, ThreadContext, Word, make_string, pop_root, push_root};
use ncl_packages::{PackageListOperations, PackageOperations};

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string allocation: {error:?}"))
}

#[test]
fn nickname_registration_updates_index_and_is_idempotent() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let base = Package::new(&mut ctx, &runtime, "BASE")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"))
        .as_word();
    let mut nickname = string(&mut ctx, &runtime, "B");
    let nickname_token = push_root(&mut ctx, &mut nickname);
    Package::from_word(base)
        .add_nickname(&mut ctx, &runtime, nickname)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert!(
        !Package::from_word(base)
            .add_nickname(&mut ctx, &runtime, nickname)
            .unwrap_or_else(|error| panic!("test failure: {error:?}"))
    );
    let mut registered = runtime
        .ensure_package(&mut ctx, "REGISTERED")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let token = push_root(&mut ctx, &mut registered);
    Package::from_word(registered)
        .add_nickname(&mut ctx, &runtime, nickname)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert_eq!(runtime.find_package(&ctx, "B"), Some(registered));
    assert_eq!(
        Package::from_word(registered).remove_nickname(&mut ctx, &runtime, nickname),
        Ok(true)
    );
    assert_eq!(runtime.find_package(&ctx, "B"), None);
    let mut replacement = runtime
        .ensure_package(&mut ctx, "REGISTERED-REPLACEMENT")
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    let replacement_token = push_root(&mut ctx, &mut replacement);
    Package::from_word(replacement)
        .add_nickname(&mut ctx, &runtime, nickname)
        .unwrap_or_else(|error| panic!("test failure: {error:?}"));
    assert_eq!(runtime.find_package(&ctx, "B"), Some(replacement));
    assert!(pop_root(&mut ctx, replacement_token));
    assert!(pop_root(&mut ctx, token));
    assert!(pop_root(&mut ctx, nickname_token));
}

#[test]
fn removing_primary_name_nickname_keeps_primary_lookup() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let mut package = runtime
        .ensure_package(&mut ctx, "PRIMARY-NAME")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let package_token = push_root(&mut ctx, &mut package);
    let mut nickname = string(&mut ctx, &runtime, "PRIMARY-NAME");
    let nickname_token = push_root(&mut ctx, &mut nickname);
    assert!(
        Package::from_word(package)
            .add_nickname(&mut ctx, &runtime, nickname)
            .unwrap_or_else(|error| panic!("add nickname: {error:?}"))
    );
    assert_eq!(runtime.find_package(&ctx, "PRIMARY-NAME"), Some(package));
    assert!(
        Package::from_word(package)
            .remove_nickname(&mut ctx, &runtime, nickname)
            .unwrap_or_else(|error| panic!("remove nickname: {error:?}"))
    );
    assert_eq!(runtime.find_package(&ctx, "PRIMARY-NAME"), Some(package));
    assert!(pop_root(&mut ctx, nickname_token));
    assert!(pop_root(&mut ctx, package_token));
}

#[test]
fn equal_but_distinct_nicknames_keep_the_index_until_the_last_is_removed() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let mut package = runtime
        .ensure_package(&mut ctx, "DUPLICATE-NICKNAMES")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let package_token = push_root(&mut ctx, &mut package);
    let mut first = string(&mut ctx, &runtime, "DUP");
    let first_token = push_root(&mut ctx, &mut first);
    let mut second = string(&mut ctx, &runtime, "DUP");
    let second_token = push_root(&mut ctx, &mut second);
    assert_ne!(first, second);

    assert!(
        Package::from_word(package)
            .add_nickname(&mut ctx, &runtime, first)
            .unwrap_or_else(|error| panic!("first nickname: {error:?}"))
    );
    assert!(
        Package::from_word(package)
            .add_nickname(&mut ctx, &runtime, second)
            .unwrap_or_else(|error| panic!("equal nickname: {error:?}"))
    );
    assert_eq!(runtime.find_package(&ctx, "DUP"), Some(package));
    let mut different = string(&mut ctx, &runtime, "DUQ");
    let different_token = push_root(&mut ctx, &mut different);
    let mut different_length = string(&mut ctx, &runtime, "LONGER");
    let different_length_token = push_root(&mut ctx, &mut different_length);
    assert_eq!(
        Package::from_word(package).remove_nickname(&mut ctx, &runtime, different),
        Ok(false)
    );
    assert_eq!(
        Package::from_word(package).remove_nickname(&mut ctx, &runtime, different_length),
        Ok(false)
    );
    assert_eq!(runtime.find_package(&ctx, "DUP"), Some(package));

    assert_eq!(
        Package::from_word(package).remove_nickname(&mut ctx, &runtime, first),
        Ok(true)
    );
    assert_eq!(runtime.find_package(&ctx, "DUP"), Some(package));
    assert_eq!(
        Package::from_word(package).remove_nickname(&mut ctx, &runtime, second),
        Ok(true)
    );
    assert_eq!(runtime.find_package(&ctx, "DUP"), None);

    assert!(pop_root(&mut ctx, different_length_token));
    assert!(pop_root(&mut ctx, different_token));
    assert!(pop_root(&mut ctx, second_token));
    assert!(pop_root(&mut ctx, first_token));
    assert!(pop_root(&mut ctx, package_token));
}

#[test]
fn conflicting_registered_nickname_leaves_the_original_owner_unchanged() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let mut owner = runtime
        .ensure_package(&mut ctx, "NICKNAME-OWNER")
        .unwrap_or_else(|error| panic!("owner: {error:?}"));
    let owner_token = push_root(&mut ctx, &mut owner);
    let mut other = runtime
        .ensure_package(&mut ctx, "NICKNAME-OTHER")
        .unwrap_or_else(|error| panic!("other: {error:?}"));
    let other_token = push_root(&mut ctx, &mut other);
    let mut nickname = string(&mut ctx, &runtime, "NICKNAME-OWNER");
    let nickname_token = push_root(&mut ctx, &mut nickname);

    assert_eq!(
        Package::from_word(other).add_nickname(&mut ctx, &runtime, nickname),
        Err(ObjectError::PackageConflict)
    );
    assert_eq!(
        Package::from_word(other)
            .nicknames(&ctx)
            .unwrap_or_else(|error| panic!("nicknames: {error:?}")),
        Word::NIL
    );
    assert_eq!(runtime.find_package(&ctx, "NICKNAME-OWNER"), Some(owner));

    assert!(pop_root(&mut ctx, nickname_token));
    assert!(pop_root(&mut ctx, other_token));
    assert!(pop_root(&mut ctx, owner_token));
}

#[test]
fn try_from_word_rejects_non_package_values() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    assert_eq!(
        Package::try_from_word(&ctx, Word::fixnum(25)),
        Err(ObjectError::TypeError)
    );
}

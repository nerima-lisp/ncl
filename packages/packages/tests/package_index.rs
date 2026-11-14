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
    let mut name = Package::from_word(package)
        .name(&ctx)
        .unwrap_or_else(|error| panic!("package name: {error:?}"));
    let name_token = push_root(&mut ctx, &mut name);
    assert!(
        Package::from_word(package)
            .add_nickname(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("add nickname: {error:?}"))
    );
    assert_eq!(runtime.find_package(&ctx, "PRIMARY-NAME"), Some(package));
    assert!(
        Package::from_word(package)
            .remove_nickname(&mut ctx, &runtime, name)
            .unwrap_or_else(|error| panic!("remove nickname: {error:?}"))
    );
    assert_eq!(runtime.find_package(&ctx, "PRIMARY-NAME"), Some(package));
    assert!(pop_root(&mut ctx, name_token));
    assert!(pop_root(&mut ctx, package_token));
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

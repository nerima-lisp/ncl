//! Runtime package-name index lifecycle tests.

use ncl_object::{
    ObjectError, Package, Runtime, ThreadContext, Word, make_string, pop_root, push_root,
};

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string: {error:?}"))
}

#[test]
fn registration_and_owner_checked_removal_keep_the_index_consistent() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let mut owner = runtime
        .ensure_package(&mut ctx, "INDEX-OWNER")
        .unwrap_or_else(|error| panic!("owner: {error:?}"));
    let owner_token = push_root(&mut ctx, &mut owner);
    let mut other = Package::new(&mut ctx, &runtime, "INDEX-OTHER")
        .unwrap_or_else(|error| panic!("other: {error:?}"))
        .as_word();
    let other_token = push_root(&mut ctx, &mut other);
    let mut nickname = string(&mut ctx, &runtime, "INDEX-ALIAS");
    let nickname_token = push_root(&mut ctx, &mut nickname);

    assert_eq!(runtime.package_is_registered(&mut ctx, owner), Ok(true));
    assert_eq!(
        runtime.register_package_name(&mut ctx, owner, nickname),
        Ok(())
    );
    assert_eq!(
        runtime.register_package_name(&mut ctx, owner, nickname),
        Ok(())
    );
    assert_eq!(runtime.find_package(&ctx, "INDEX-ALIAS"), Some(owner));
    assert_eq!(
        runtime.register_package_name(&mut ctx, other, nickname),
        Err(ObjectError::PackageConflict)
    );
    assert_eq!(
        runtime.unregister_package_name_for_package(&mut ctx, other, nickname),
        Ok(false)
    );
    assert_eq!(runtime.find_package(&ctx, "INDEX-ALIAS"), Some(owner));
    assert_eq!(
        runtime.unregister_package_name_for_package(&mut ctx, owner, nickname),
        Ok(true)
    );
    assert_eq!(runtime.find_package(&ctx, "INDEX-ALIAS"), None);
    assert_eq!(
        runtime.unregister_package_name_for_package(&mut ctx, owner, nickname),
        Ok(false)
    );
    assert_eq!(
        runtime.unregister_package_name(&mut ctx, nickname),
        Ok(false)
    );

    let mut primary = Package::from_word(owner)
        .name(&ctx)
        .unwrap_or_else(|error| panic!("primary name: {error:?}"));
    let primary_token = push_root(&mut ctx, &mut primary);
    assert_eq!(
        runtime.unregister_package_name_for_package(&mut ctx, owner, primary),
        Ok(true)
    );
    assert_eq!(runtime.package_is_registered(&mut ctx, owner), Ok(false));
    assert_eq!(
        runtime.register_package_name(&mut ctx, owner, primary),
        Ok(())
    );
    assert_eq!(runtime.package_is_registered(&mut ctx, owner), Ok(true));
    assert_eq!(runtime.find_package(&ctx, "INDEX-OWNER"), Some(owner));

    assert!(pop_root(&mut ctx, primary_token));
    assert!(pop_root(&mut ctx, nickname_token));
    assert!(pop_root(&mut ctx, other_token));
    assert!(pop_root(&mut ctx, owner_token));
}

#[test]
fn sorted_index_keeps_neighbors_after_middle_name_removal_and_reinsertion() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));

    let mut alpha_owner = Package::new(&mut ctx, &runtime, "SORTED-OWNER-A")
        .unwrap_or_else(|error| panic!("alpha owner: {error:?}"))
        .as_word();
    let alpha_root = push_root(&mut ctx, &mut alpha_owner);
    let mut middle_owner = Package::new(&mut ctx, &runtime, "SORTED-OWNER-M")
        .unwrap_or_else(|error| panic!("middle owner: {error:?}"))
        .as_word();
    let middle_root = push_root(&mut ctx, &mut middle_owner);
    let mut zulu_owner = Package::new(&mut ctx, &runtime, "SORTED-OWNER-Z")
        .unwrap_or_else(|error| panic!("zulu owner: {error:?}"))
        .as_word();
    let zulu_root = push_root(&mut ctx, &mut zulu_owner);

    for (name, owner) in [
        ("SORTED-Z", zulu_owner),
        ("SORTED-A", alpha_owner),
        ("SORTED-M", middle_owner),
    ] {
        let name = string(&mut ctx, &runtime, name);
        assert_eq!(runtime.register_package_name(&mut ctx, owner, name), Ok(()));
    }

    assert_eq!(runtime.find_package(&ctx, "SORTED-Z"), Some(zulu_owner));
    assert_eq!(runtime.find_package(&ctx, "SORTED-A"), Some(alpha_owner));
    assert_eq!(runtime.find_package(&ctx, "SORTED-M"), Some(middle_owner));

    let middle_name = string(&mut ctx, &runtime, "SORTED-M");
    assert_eq!(
        runtime.unregister_package_name_for_package(&mut ctx, middle_owner, middle_name),
        Ok(true)
    );
    assert_eq!(runtime.find_package(&ctx, "SORTED-M"), None);
    assert_eq!(runtime.find_package(&ctx, "SORTED-A"), Some(alpha_owner));
    assert_eq!(runtime.find_package(&ctx, "SORTED-Z"), Some(zulu_owner));

    let mut recreated = runtime
        .ensure_package(&mut ctx, "SORTED-M")
        .unwrap_or_else(|error| panic!("recreated package: {error:?}"));
    let recreated_token = push_root(&mut ctx, &mut recreated);
    assert_ne!(recreated, middle_owner);
    assert_eq!(runtime.find_package(&ctx, "SORTED-M"), Some(recreated));
    assert_eq!(runtime.find_package(&ctx, "SORTED-A"), Some(alpha_owner));
    assert_eq!(runtime.find_package(&ctx, "SORTED-Z"), Some(zulu_owner));

    assert!(pop_root(&mut ctx, recreated_token));
    assert!(pop_root(&mut ctx, zulu_root));
    assert!(pop_root(&mut ctx, middle_root));
    assert!(pop_root(&mut ctx, alpha_root));
}

#[test]
fn ensure_package_restores_a_removed_standard_nickname_registration() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .unwrap_or_else(|| panic!("COMMON-LISP package was not found"));
    let nickname = string(&mut ctx, &runtime, "CL");

    assert_eq!(
        runtime.unregister_package_name(&mut ctx, nickname),
        Ok(true)
    );
    assert_eq!(runtime.find_package(&ctx, "CL"), None);
    assert_eq!(runtime.ensure_package(&mut ctx, "CL"), Ok(common_lisp));
    assert_eq!(runtime.find_package(&ctx, "CL"), Some(common_lisp));
}

#[test]
fn non_string_package_name_is_rejected_without_changing_the_registry() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = runtime
        .ensure_package(&mut ctx, "INDEX-NONSTRING-NAME")
        .unwrap_or_else(|error| panic!("package: {error:?}"));

    assert_eq!(
        runtime.register_package_name(&mut ctx, package, Word::fixnum(42)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.find_package(&ctx, "INDEX-NONSTRING-NAME"),
        Some(package)
    );
}

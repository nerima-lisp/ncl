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

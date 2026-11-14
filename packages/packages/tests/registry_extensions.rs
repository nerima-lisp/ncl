//! Integration tests for package registry extension and conflict behavior.

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, make_cons, make_string};
use ncl_packages::{PackageOperations, RuntimePackageOperations};

#[test]
fn common_lisp_nickname_collision_is_rejected_without_overwriting() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    runtime.delete_package(&mut ctx, Package::from_word(common_lisp))?;
    let cl_owner = runtime.ensure_package(&mut ctx, "N25-CL-OWNER")?;
    let cl_name = make_string(
        &mut ctx,
        &runtime,
        &['N', '2', '5', '-', 'C', 'L', '-', 'O', 'W', 'N', 'E', 'R'],
    )?;
    let nickname = make_string(&mut ctx, &runtime, &['C', 'L'])?;
    let nicknames = ncl_object::make_cons(&mut ctx, &runtime, nickname, ncl_object::Word::NIL)?;
    runtime.rename_package(&mut ctx, Package::from_word(cl_owner), cl_name, nicknames)?;

    assert_eq!(
        runtime.ensure_package(&mut ctx, "COMMON-LISP"),
        Err(ObjectError::PackageConflict)
    );
    assert_eq!(runtime.find_package(&ctx, "CL"), Some(cl_owner));
    assert_eq!(runtime.find_package(&ctx, "COMMON-LISP"), None);
    Ok(())
}

#[test]
fn ensure_package_does_not_resolve_a_canonical_name_to_its_nickname_owner()
-> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let mut common_lisp = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    let common_lisp_token = ncl_object::push_root(&mut ctx, &mut common_lisp);
    let mut primary_name = Package::from_word(common_lisp).name(&ctx)?;
    let primary_name_token = ncl_object::push_root(&mut ctx, &mut primary_name);
    assert!(runtime.unregister_package_name(&mut ctx, primary_name)?);

    let mut nickname_owner = runtime.ensure_package(&mut ctx, "N25-CANONICAL-NICKNAME-OWNER")?;
    let nickname_owner_token = ncl_object::push_root(&mut ctx, &mut nickname_owner);
    let nickname = make_string(
        &mut ctx,
        &runtime,
        &"COMMON-LISP".chars().collect::<Vec<_>>(),
    )?;
    assert!(Package::from_word(nickname_owner).add_nickname(&mut ctx, &runtime, nickname)?);

    assert_eq!(
        runtime.find_package(&ctx, "COMMON-LISP"),
        Some(nickname_owner)
    );
    assert_eq!(runtime.find_package(&ctx, "CL"), Some(common_lisp));
    assert_eq!(
        runtime.ensure_package(&mut ctx, "COMMON-LISP"),
        Err(ObjectError::PackageConflict)
    );
    assert_eq!(
        runtime.find_package(&ctx, "COMMON-LISP"),
        Some(nickname_owner)
    );
    assert_eq!(runtime.find_package(&ctx, "CL"), Some(common_lisp));

    assert!(ncl_object::pop_root(&mut ctx, nickname_owner_token));
    assert!(ncl_object::pop_root(&mut ctx, primary_name_token));
    assert!(ncl_object::pop_root(&mut ctx, common_lisp_token));
    Ok(())
}

#[test]
fn package_name_and_nickname_collisions_are_rejected() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let first = runtime.ensure_package(&mut ctx, "N25-REGISTRY-FIRST")?;
    let second = runtime.ensure_package(&mut ctx, "N25-REGISTRY-SECOND")?;
    let first_name = make_string(
        &mut ctx,
        &runtime,
        &"N25-REGISTRY-FIRST".chars().collect::<Vec<_>>(),
    )?;
    assert_eq!(
        runtime.rename_package(&mut ctx, Package::from_word(second), first_name, Word::NIL),
        Err(ObjectError::PackageConflict)
    );

    let renamed_first = make_string(
        &mut ctx,
        &runtime,
        &"N25-REGISTRY-FIRST-RENAMED".chars().collect::<Vec<_>>(),
    )?;
    let nickname = make_string(
        &mut ctx,
        &runtime,
        &"N25-REGISTRY-NICK".chars().collect::<Vec<_>>(),
    )?;
    let nickname_list = make_cons(&mut ctx, &runtime, nickname, Word::NIL)?;
    runtime.rename_package(
        &mut ctx,
        Package::from_word(first),
        renamed_first,
        nickname_list,
    )?;

    assert_eq!(
        runtime.rename_package(
            &mut ctx,
            Package::from_word(second),
            first_name,
            nickname_list,
        ),
        Err(ObjectError::PackageConflict)
    );
    assert_eq!(
        runtime.find_package(&ctx, "N25-REGISTRY-FIRST-RENAMED"),
        Some(first)
    );
    Ok(())
}

#![allow(missing_docs)]

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, make_cons, make_string};

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

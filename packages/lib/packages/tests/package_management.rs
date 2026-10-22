#![allow(missing_docs)]

use ncl_lib_packages::register as register_package_builtins;
use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_cons, make_string,
    pop_root, push_root,
};

#[test]
fn locked_delete_leaves_the_package_object_and_registered_names_unchanged()
-> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register_package_builtins(&runtime)?;

    let package = runtime.ensure_package(&mut ctx, "OO35-LOCKED-DELETE-OLD")?;
    let mut rooted_package = package;
    let package_root = push_root(&mut ctx, &mut rooted_package);
    let nickname = make_string(
        &mut ctx,
        &runtime,
        &"OO35-LOCKED-DELETE-ALIAS".chars().collect::<Vec<_>>(),
    )?;
    let nicknames = make_cons(&mut ctx, &runtime, nickname, Word::NIL)?;
    let name = make_string(
        &mut ctx,
        &runtime,
        &"OO35-LOCKED-DELETE".chars().collect::<Vec<_>>(),
    )?;
    runtime.rename_package(
        &mut ctx,
        Package::from_word(rooted_package),
        name,
        nicknames,
    )?;

    let lock = runtime
        .function(&mut ctx, "NCL-EXT", "LOCK-PACKAGE")
        .ok_or(ObjectError::UndefinedFunction)?;
    let delete = runtime
        .function(&mut ctx, "COMMON-LISP", "DELETE-PACKAGE")
        .ok_or(ObjectError::UndefinedFunction)?;
    let lock = FunctionObject::try_from(lock)?;
    let delete = FunctionObject::try_from(delete)?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, lock, &[rooted_package]),
        Ok(rooted_package)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, delete, &[rooted_package]),
        Err(ObjectError::TypeError)
    );

    let package = Package::from_word(rooted_package);
    assert_eq!(package.is_locked(&ctx), Ok(true));
    assert_eq!(package.name(&ctx), Ok(name));
    assert_eq!(package.nicknames(&ctx), Ok(nicknames));
    assert_eq!(
        runtime.find_package(&ctx, "OO35-LOCKED-DELETE"),
        Some(rooted_package)
    );
    assert_eq!(
        runtime.find_package(&ctx, "OO35-LOCKED-DELETE-ALIAS"),
        Some(rooted_package)
    );

    assert!(pop_root(&mut ctx, package_root));
    Ok(())
}

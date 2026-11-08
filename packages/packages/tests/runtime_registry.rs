//! Package registry lifecycle and runtime registry regression tests.

use ncl_object::{ObjectError, Package, Runtime, ThreadContext, Word, make_cons, make_string};
use ncl_packages::RuntimePackageOperations;

#[test]
fn package_lifecycle_and_class_registry_preserve_names() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let common_lisp = runtime.ensure_package(&mut ctx, "CL")?;
    assert_eq!(
        runtime.ensure_package(&mut ctx, "COMMON-LISP")?,
        common_lisp
    );
    assert_eq!(runtime.find_package(&ctx, "CL"), Some(common_lisp));

    let package = runtime.ensure_package(&mut ctx, "REGISTRY-LIFECYCLE")?;
    let nickname = make_string(&mut ctx, &runtime, &['R', 'L'])?;
    let nicknames = make_cons(&mut ctx, &runtime, nickname, Word::NIL)?;
    let renamed = make_string(&mut ctx, &runtime, &['R', 'E', 'N', 'A', 'M', 'E', 'D'])?;
    runtime.rename_package(&mut ctx, Package::from_word(package), renamed, nicknames)?;
    assert_eq!(runtime.find_package(&ctx, "REGISTRY-LIFECYCLE"), None);
    assert_eq!(runtime.find_package(&ctx, "RENAMED"), Some(package));
    assert_eq!(runtime.find_package(&ctx, "RL"), Some(package));

    runtime.define_class(&mut ctx, "REGISTRY-CLASS", Word::TRUE)?;
    assert_eq!(runtime.class(&mut ctx, "REGISTRY-CLASS"), Some(Word::TRUE));
    assert_eq!(runtime.class(&mut ctx, "UNKNOWN-CLASS"), None);
    runtime.add_feature("REGISTRY-FEATURE");
    runtime.add_feature("REGISTRY-FEATURE");
    assert_eq!(runtime.features(), vec!["REGISTRY-FEATURE"]);

    assert!(runtime.all_packages(&ctx)?.contains(&package));
    assert!(runtime.delete_package(&mut ctx, Package::from_word(package))?);
    assert_eq!(runtime.find_package(&ctx, "RENAMED"), None);
    assert_eq!(runtime.find_package(&ctx, "RL"), None);
    assert!(!runtime.all_packages(&ctx)?.contains(&package));
    assert!(!runtime.delete_package(&mut ctx, Package::from_word(package))?);
    Ok(())
}

#[test]
fn common_lisp_user_has_standard_cl_user_nickname() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let common_lisp_user = runtime
        .find_package(&ctx, "COMMON-LISP-USER")
        .ok_or(ObjectError::Layout)?;

    assert_eq!(
        runtime.find_package(&ctx, "CL-USER"),
        Some(common_lisp_user)
    );
    Ok(())
}

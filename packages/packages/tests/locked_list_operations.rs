//! Package lock behavior for list operations.

use ncl_object::{LispError, ObjectError, Package, PackageError, Runtime, ThreadContext};
use ncl_packages::PackageListOperations;

fn setup() -> (Runtime, ThreadContext, Package) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = Package::new(&mut ctx, &runtime, "LOCKED-LISTS")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    (runtime, ctx, package)
}

#[test]
fn locked_shadow_rejects_with_typed_package_error() {
    let (runtime, mut ctx, package) = setup();
    let name = ncl_object::make_string(&mut ctx, &runtime, &['N'])
        .unwrap_or_else(|error| panic!("name: {error:?}"));
    package
        .set_locked(&mut ctx, true)
        .unwrap_or_else(|error| panic!("lock: {error:?}"));

    assert_eq!(
        package.shadow(&mut ctx, &runtime, name),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(LispError::PackageError(PackageError::Locked))
    );
}

#[test]
fn locked_use_package_rejects_with_typed_package_error() {
    let (runtime, mut ctx, package) = setup();
    let source = Package::new(&mut ctx, &runtime, "SOURCE-LISTS")
        .unwrap_or_else(|error| panic!("source: {error:?}"));
    package
        .set_locked(&mut ctx, true)
        .unwrap_or_else(|error| panic!("lock: {error:?}"));

    assert_eq!(
        package.use_package(&mut ctx, &runtime, source.as_word()),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(LispError::PackageError(PackageError::Locked))
    );
}

#[test]
fn locked_unuse_package_rejects_with_typed_package_error() {
    let (runtime, mut ctx, package) = setup();
    let source = Package::new(&mut ctx, &runtime, "SOURCE-LISTS")
        .unwrap_or_else(|error| panic!("source: {error:?}"));
    package
        .use_package(&mut ctx, &runtime, source.as_word())
        .unwrap_or_else(|error| panic!("use setup: {error:?}"));
    package
        .set_locked(&mut ctx, true)
        .unwrap_or_else(|error| panic!("lock: {error:?}"));

    assert_eq!(
        package.unuse_package(&mut ctx, source.as_word()),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(LispError::PackageError(PackageError::Locked))
    );
}

#[test]
fn locked_target_rejects_use_package_with_typed_package_error() {
    let (runtime, mut ctx, package) = setup();
    let target = Package::new(&mut ctx, &runtime, "TARGET-LISTS")
        .unwrap_or_else(|error| panic!("target: {error:?}"));
    target
        .set_locked(&mut ctx, true)
        .unwrap_or_else(|error| panic!("lock: {error:?}"));

    assert_eq!(
        package.use_package(&mut ctx, &runtime, target.as_word()),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(LispError::PackageError(PackageError::Locked))
    );
}

#[test]
fn locked_target_rejects_unuse_package_with_typed_package_error() {
    let (runtime, mut ctx, package) = setup();
    let target = Package::new(&mut ctx, &runtime, "TARGET-LISTS")
        .unwrap_or_else(|error| panic!("target: {error:?}"));
    package
        .use_package(&mut ctx, &runtime, target.as_word())
        .unwrap_or_else(|error| panic!("use setup: {error:?}"));
    target
        .set_locked(&mut ctx, true)
        .unwrap_or_else(|error| panic!("lock: {error:?}"));

    assert_eq!(
        package.unuse_package(&mut ctx, target.as_word()),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ctx.take_pending_lisp_error(),
        Some(LispError::PackageError(PackageError::Locked))
    );
}

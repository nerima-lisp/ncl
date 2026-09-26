//! Package-lock mutation regression coverage.

use ncl_object::{
    LispError, ObjectError, Package, PackageError, Runtime, ThreadContext, make_string,
};

#[test]
fn locked_package_rejects_namespace_mutations_with_typed_error() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = Package::new(&mut ctx, &runtime, "LOCKED")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let source = Package::new(&mut ctx, &runtime, "SOURCE")
        .unwrap_or_else(|error| panic!("source: {error:?}"));
    let name = make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E'])
        .unwrap_or_else(|error| panic!("name: {error:?}"));
    let nickname = make_string(&mut ctx, &runtime, &['N', 'I', 'C', 'K'])
        .unwrap_or_else(|error| panic!("nickname: {error:?}"));
    let (symbol, _) = source
        .intern(&mut ctx, &runtime, "NAME")
        .unwrap_or_else(|error| panic!("symbol: {error:?}"));
    package
        .add_nickname(&mut ctx, &runtime, nickname)
        .unwrap_or_else(|error| panic!("nickname setup: {error:?}"));
    package
        .set_locked(&mut ctx, true)
        .unwrap_or_else(|error| panic!("lock: {error:?}"));
    let expected = Some(LispError::PackageError(PackageError::Locked));

    assert_eq!(
        package.intern(&mut ctx, &runtime, "NEW"),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), expected);
    assert_eq!(
        package.unintern(&mut ctx, &runtime, name),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), expected);
    assert_eq!(
        package.export(&mut ctx, &runtime, name),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), expected);
    assert_eq!(
        package.unexport(&mut ctx, &runtime, name),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), expected);
    assert_eq!(
        package.import(&mut ctx, &runtime, name, symbol),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), expected);
    assert_eq!(
        package.add_nickname(&mut ctx, &runtime, nickname),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), expected);
    assert_eq!(
        package.remove_nickname(&mut ctx, nickname),
        Err(ObjectError::TypeError)
    );
    assert_eq!(ctx.take_pending_lisp_error(), expected);
}

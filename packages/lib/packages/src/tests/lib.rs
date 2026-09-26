use super::*;
use ncl_object::FunctionObject;

#[test]
fn lock_operations_round_trip() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("thread: {error:?}"));
    register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = runtime
        .ensure_package(&mut ctx, "N25-LOCK")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let lock = runtime
        .function(&mut ctx, "NCL-EXT", "LOCK-PACKAGE")
        .unwrap_or_else(|| panic!("lock"));
    let locked = runtime
        .function(&mut ctx, "NCL-EXT", "PACKAGE-LOCKED-P")
        .unwrap_or_else(|| panic!("locked"));
    let unlock = runtime
        .function(&mut ctx, "NCL-EXT", "UNLOCK-PACKAGE")
        .unwrap_or_else(|| panic!("unlock"));
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(lock).unwrap_or_else(|e| panic!("lock fn: {e:?}")),
            &[package]
        ),
        Ok(package)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(locked).unwrap_or_else(|e| panic!("locked fn: {e:?}")),
            &[package]
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(unlock).unwrap_or_else(|e| panic!("unlock fn: {e:?}")),
            &[package]
        ),
        Ok(package)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(locked).unwrap_or_else(|e| panic!("locked fn: {e:?}")),
            &[package]
        ),
        Ok(Word::NIL)
    );
}

#[test]
fn package_local_nickname_builtins_round_trip() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("thread: {error:?}"));
    register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = runtime
        .ensure_package(&mut ctx, "N25-PLN-OWNER")
        .unwrap_or_else(|error| panic!("owner: {error:?}"));
    let target = runtime
        .ensure_package(&mut ctx, "N25-PLN-TARGET")
        .unwrap_or_else(|error| panic!("target: {error:?}"));
    let nickname = ncl_object::make_string(&mut ctx, &runtime, &['L', 'O', 'C', 'A', 'L'])
        .unwrap_or_else(|error| panic!("nickname: {error:?}"));
    let add = runtime
        .function(&mut ctx, "NCL-EXT", "ADD-PACKAGE-LOCAL-NICKNAME")
        .unwrap_or_else(|| panic!("add"));
    let remove = runtime
        .function(&mut ctx, "NCL-EXT", "REMOVE-PACKAGE-LOCAL-NICKNAME")
        .unwrap_or_else(|| panic!("remove"));
    let list = runtime
        .function(&mut ctx, "NCL-EXT", "PACKAGE-LOCAL-NICKNAMES")
        .unwrap_or_else(|| panic!("list"));

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(add).unwrap_or_else(|error| panic!("add fn: {error:?}")),
            &[nickname, target, package],
        ),
        Ok(nickname)
    );
    let entries = runtime
        .call_builtin(
            &mut ctx,
            FunctionObject::try_from(list).unwrap_or_else(|error| panic!("list fn: {error:?}")),
            &[package],
        )
        .unwrap_or_else(|error| panic!("list result: {error:?}"));
    let entry =
        ncl_object::car(&mut ctx, entries).unwrap_or_else(|error| panic!("entry: {error:?}"));
    assert_eq!(ncl_object::car(&mut ctx, entry), Ok(nickname));
    assert_eq!(ncl_object::cdr(&mut ctx, entry), Ok(target));
    assert_eq!(
        Package::from_word(package)
            .resolve_local_nickname(&ctx, StringObject::from_word(nickname))
            .unwrap_or_else(|error| panic!("resolve: {error:?}")),
        Some(Package::from_word(target))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(remove)
                .unwrap_or_else(|error| panic!("remove fn: {error:?}")),
            &[nickname, package],
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(list).unwrap_or_else(|error| panic!("list fn: {error:?}")),
            &[package],
        ),
        Ok(Word::NIL)
    );
}

#[test]
fn package_local_nickname_builtins_reject_locked_package() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("thread: {error:?}"));
    register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = runtime
        .ensure_package(&mut ctx, "N25-PLN-LOCKED")
        .unwrap_or_else(|error| panic!("owner: {error:?}"));
    let target = runtime
        .ensure_package(&mut ctx, "N25-PLN-LOCK-TARGET")
        .unwrap_or_else(|error| panic!("target: {error:?}"));
    let nickname = ncl_object::make_string(&mut ctx, &runtime, &['L', 'O', 'C', 'K'])
        .unwrap_or_else(|error| panic!("nickname: {error:?}"));
    let lock = runtime
        .function(&mut ctx, "NCL-EXT", "LOCK-PACKAGE")
        .unwrap_or_else(|| panic!("lock"));
    let add = runtime
        .function(&mut ctx, "NCL-EXT", "ADD-PACKAGE-LOCAL-NICKNAME")
        .unwrap_or_else(|| panic!("add"));
    let remove = runtime
        .function(&mut ctx, "NCL-EXT", "REMOVE-PACKAGE-LOCAL-NICKNAME")
        .unwrap_or_else(|| panic!("remove"));
    let function = |word| {
        FunctionObject::try_from(word).unwrap_or_else(|error| panic!("function: {error:?}"))
    };

    assert_eq!(
        runtime.call_builtin(&mut ctx, function(lock), &[package]),
        Ok(package)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, function(add), &[nickname, target, package]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, function(remove), &[nickname, package]),
        Err(ObjectError::TypeError)
    );
}

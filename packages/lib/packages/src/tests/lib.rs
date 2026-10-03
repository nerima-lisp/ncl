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
    let entry = ncl_object::car(&ctx, entries).unwrap_or_else(|error| panic!("entry: {error:?}"));
    assert_eq!(ncl_object::car(&ctx, entry), Ok(nickname));
    assert_eq!(ncl_object::cdr(&ctx, entry), Ok(target));
    assert_eq!(
        Package::from_word(package)
            .resolve_local_nickname(&ctx, StringObject::from_word(nickname))
            .unwrap_or_else(|error| panic!("resolve: {error:?}")),
        Some(Package::from_word(target))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            FunctionObject::try_from(remove).unwrap_or_else(|error| panic!("remove fn: {error:?}")),
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
    let function =
        |word| FunctionObject::try_from(word).unwrap_or_else(|error| panic!("function: {error:?}"));

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

fn make_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: &str,
) -> Result<Word, ObjectError> {
    ncl_object::make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
}

fn package_function(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
) -> Result<FunctionObject, ObjectError> {
    runtime
        .function(ctx, "NCL-EXT", name)
        .ok_or(ObjectError::Layout)
        .and_then(FunctionObject::try_from)
}

#[test]
fn package_local_nickname_designators_and_removal_edges_are_value_based() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;

    let owner = runtime.ensure_package(&mut ctx, "N25-DESIGNATOR-OWNER")?;
    let first_target = runtime.ensure_package(&mut ctx, "N25-DESIGNATOR-FIRST")?;
    let _second_target = runtime.ensure_package(&mut ctx, "N25-DESIGNATOR-SECOND")?;
    let _third_target = runtime.ensure_package(&mut ctx, "N25-DESIGNATOR-THIRD")?;
    let owner_name = make_string(&mut ctx, &runtime, "N25-DESIGNATOR-OWNER")?;
    let first_target_name = make_string(&mut ctx, &runtime, "N25-DESIGNATOR-FIRST")?;
    let second_target_name = make_string(&mut ctx, &runtime, "N25-DESIGNATOR-SECOND")?;
    let third_target_name = make_string(&mut ctx, &runtime, "N25-DESIGNATOR-THIRD")?;
    let first_name = make_string(&mut ctx, &runtime, "FIRST")?;
    let second_name = make_string(&mut ctx, &runtime, "SECOND")?;
    let third_name = make_string(&mut ctx, &runtime, "THIRD")?;
    let owner_symbol = ncl_object::make_symbol(&mut ctx, &runtime, owner_name)?;
    let second_target_symbol = ncl_object::make_symbol(&mut ctx, &runtime, second_target_name)?;
    let second_nickname_symbol = ncl_object::make_symbol(&mut ctx, &runtime, second_name)?;

    let add = package_function(&runtime, &mut ctx, "ADD-PACKAGE-LOCAL-NICKNAME")?;
    let remove = package_function(&runtime, &mut ctx, "REMOVE-PACKAGE-LOCAL-NICKNAME")?;
    let list = package_function(&runtime, &mut ctx, "PACKAGE-LOCAL-NICKNAMES")?;

    let cases = [
        (
            "string designators",
            [first_name, first_target_name, owner_name],
            first_name,
        ),
        (
            "symbol/package designators",
            [second_nickname_symbol, second_target_symbol, owner_symbol],
            second_name,
        ),
    ];
    for (label, arguments, expected) in cases {
        assert_eq!(
            runtime.call_builtin(&mut ctx, add, &arguments),
            Ok(expected),
            "add {label}"
        );
    }
    assert_eq!(
        runtime.call_builtin(&mut ctx, add, &[first_name, first_target, owner]),
        Err(ObjectError::TypeError)
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, add, &[third_name, third_target_name, owner]),
        Ok(third_name)
    );
    let entries = runtime.call_builtin(&mut ctx, list, &[owner_symbol])?;
    let entries = introspection::list_items(&ctx, entries)?;
    let entry_names = entries
        .iter()
        .map(|entry| ncl_object::car(&ctx, *entry))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(entry_names, vec![third_name, second_name, first_name]);

    assert_eq!(
        runtime.call_builtin(&mut ctx, remove, &[second_nickname_symbol, owner_name]),
        Ok(Word::TRUE)
    );
    let entries = runtime.call_builtin(&mut ctx, list, &[owner])?;
    let entries = introspection::list_items(&ctx, entries)?;
    let entry_names = entries
        .iter()
        .map(|entry| ncl_object::car(&ctx, *entry))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(entry_names, vec![third_name, first_name]);
    let missing_name = make_string(&mut ctx, &runtime, "MISSING")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, remove, &[missing_name, owner]),
        Ok(Word::NIL)
    );

    let invalid_cases = [
        ("nickname", [Word::fixnum(1), first_target, owner]),
        ("target", [first_name, Word::fixnum(2), owner]),
        ("package", [first_name, first_target, Word::fixnum(3)]),
        (
            "unknown target",
            [
                first_name,
                make_string(&mut ctx, &runtime, "N25-MISSING")?,
                owner,
            ],
        ),
    ];
    for (label, arguments) in invalid_cases {
        assert_eq!(
            runtime.call_builtin(&mut ctx, add, &arguments),
            Err(ObjectError::TypeError),
            "invalid {label}"
        );
    }
    assert_eq!(
        runtime.call_builtin(&mut ctx, list, &[Word::fixnum(4)]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

use super::*;
use ncl_object::FunctionObject;

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", name)
            .ok_or(ObjectError::Layout)?,
    )?;
    runtime.call_builtin(ctx, function, args)
}

#[test]
fn introspection_and_mutation_round_trip() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("thread: {error:?}"));
    register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
    let package = runtime
        .ensure_package(&mut ctx, "N25-INTROSPECTION")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let function = |ctx: &mut ThreadContext, name| {
        FunctionObject::try_from(
            runtime
                .function(ctx, "COMMON-LISP", name)
                .unwrap_or_else(|| panic!("{name}")),
        )
        .unwrap_or_else(|error| panic!("function: {error:?}"))
    };
    let name = ncl_object::make_string(&mut ctx, &runtime, &['A'])
        .unwrap_or_else(|error| panic!("name: {error:?}"));
    let intern = function(&mut ctx, "INTERN");
    let symbol = runtime
        .call_builtin(&mut ctx, intern, &[name, package])
        .unwrap_or_else(|error| panic!("intern: {error:?}"));
    assert_eq!(ctx.values().len(), 2);
    let package_name = function(&mut ctx, "PACKAGE-NAME");
    assert_eq!(
        runtime.call_builtin(&mut ctx, package_name, &[package]),
        Ok(ncl_object::Package::from_word(package)
            .name(&ctx)
            .unwrap_or_else(|error| panic!("name: {error:?}")))
    );
    let packagep = function(&mut ctx, "PACKAGEP");
    assert_eq!(
        runtime.call_builtin(&mut ctx, packagep, &[package]),
        Ok(Word::TRUE)
    );
    let export = function(&mut ctx, "EXPORT");
    let symbols = ncl_object::make_cons(&mut ctx, &runtime, symbol, Word::NIL)
        .unwrap_or_else(|error| panic!("list: {error:?}"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, export, &[symbols, package]),
        Ok(Word::TRUE)
    );
    let find = function(&mut ctx, "FIND-SYMBOL");
    assert_eq!(
        runtime.call_builtin(&mut ctx, find, &[name, package]),
        Ok(symbol)
    );
    assert_eq!(ctx.values().len(), 2);
}

#[test]
fn make_package_builtin_creates_and_rejects_duplicate() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let function = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-PACKAGE")
            .ok_or(ObjectError::Layout)?,
    )?;
    let name = ncl_object::make_string(&mut ctx, &runtime, &['N', '2', '5', '-', 'M', 'P'])?;
    let package = runtime.call_builtin(&mut ctx, function, &[name])?;
    let package_name = Package::try_from_word(&ctx, package)?.name(&ctx)?;
    assert_eq!(ncl_object::string_length(&ctx, package_name)?, 6);
    assert_eq!(ncl_object::string_ref(&ctx, package_name, 0)?, 'N');
    assert_eq!(ncl_object::string_ref(&ctx, package_name, 5)?, 'P');
    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[name]),
        Err(ObjectError::PackageConflict)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[Word::fixnum(25)]),
        Err(ObjectError::TypeError)
    );

    let nickname_key = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &['N', 'I', 'C', 'K', 'N', 'A', 'M', 'E', 'S'],
    )?;
    let common_lisp_nickname = ncl_object::make_string(&mut ctx, &runtime, &['C', 'L'])?;
    let nicknames = ncl_object::make_cons(&mut ctx, &runtime, common_lisp_nickname, Word::NIL)?;
    let conflicting_name = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &['N', '2', '5', '-', 'C', 'L', '-', 'N', 'I', 'C', 'K'],
    )?;
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            function,
            &[conflicting_name, nickname_key, nicknames],
        ),
        Err(ObjectError::PackageConflict)
    );
    Ok(())
}

#[test]
fn package_lists_and_symbol_search_are_registered() -> Result<(), ObjectError> {
    fn function(
        runtime: &Runtime,
        ctx: &mut ThreadContext,
        name: &str,
    ) -> Result<FunctionObject, ObjectError> {
        FunctionObject::try_from(
            runtime
                .function(ctx, "COMMON-LISP", name)
                .ok_or(ObjectError::Layout)?,
        )
    }

    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let left = runtime.ensure_package(&mut ctx, "N25-LEFT")?;
    let right = runtime.ensure_package(&mut ctx, "N25-RIGHT")?;
    let left = Package::from_word(left);
    let right = Package::from_word(right);
    let shared = ncl_object::make_string(&mut ctx, &runtime, &['S', 'H', 'A', 'R', 'E', 'D'])?;
    let (symbol, _) = left.intern(&mut ctx, &runtime, "SHARED")?;
    right.import(&mut ctx, &runtime, shared, symbol)?;
    left.use_package(&mut ctx, &runtime, right.as_word())?;

    let use_list_function = function(&runtime, &mut ctx, "PACKAGE-USE-LIST")?;
    let use_list = runtime.call_builtin(&mut ctx, use_list_function, &[left.as_word()])?;
    assert_eq!(list_items(&ctx, use_list)?, vec![right.as_word()]);
    let used_by_function = function(&runtime, &mut ctx, "PACKAGE-USED-BY-LIST")?;
    let used_by = runtime.call_builtin(&mut ctx, used_by_function, &[right.as_word()])?;
    assert_eq!(list_items(&ctx, used_by)?, vec![left.as_word()]);

    let all_function = function(&runtime, &mut ctx, "LIST-ALL-PACKAGES")?;
    let all = runtime.call_builtin(&mut ctx, all_function, &[])?;
    assert!(list_items(&ctx, all)?.contains(&left.as_word()));
    let name = ncl_object::make_string(&mut ctx, &runtime, &['S', 'H', 'A', 'R', 'E', 'D'])?;
    let found_function = function(&runtime, &mut ctx, "FIND-ALL-SYMBOLS")?;
    let found = runtime.call_builtin(&mut ctx, found_function, &[name])?;
    assert_eq!(list_items(&ctx, found)?, vec![symbol]);
    Ok(())
}

#[test]
fn package_operations_survive_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let mut package = runtime.ensure_package(&mut ctx, "N25-STRESS-PACKAGE")?;
    let package_token = ncl_object::push_root(&mut ctx, &mut package);
    let mut used = runtime.ensure_package(&mut ctx, "N25-STRESS-USED")?;
    let used_token = ncl_object::push_root(&mut ctx, &mut used);
    let (symbol, status) = Package::from_word(package).intern(&mut ctx, &runtime, "STRESSED")?;
    assert_eq!(status, ncl_object::FindStatus::Internal);
    let mut symbol = symbol;
    let symbol_token = ncl_object::push_root(&mut ctx, &mut symbol);
    let export_name = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &['S', 'T', 'R', 'E', 'S', 'S', 'E', 'D'],
    )?;
    Package::from_word(package).export(&mut ctx, &runtime, export_name)?;
    Package::from_word(package).use_package(&mut ctx, &runtime, used)?;
    let lookup = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &['S', 'T', 'R', 'E', 'S', 'S', 'E', 'D'],
    )?;
    assert_eq!(
        Package::from_word(package).find_symbol(&mut ctx, lookup)?,
        Some((symbol, ncl_object::FindStatus::External))
    );

    assert!(ncl_object::pop_root(&mut ctx, symbol_token));
    assert!(ncl_object::pop_root(&mut ctx, used_token));
    assert!(ncl_object::pop_root(&mut ctx, package_token));
    Ok(())
}

#[test]
fn package_introspection_lists_survive_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let mut package = runtime.ensure_package(&mut ctx, "N25-STRESS-INTROSPECTION")?;
    let package_token = ncl_object::push_root(&mut ctx, &mut package);
    let symbol = Package::from_word(package)
        .intern(&mut ctx, &runtime, "VISIBLE")?
        .0;
    let mut symbol = symbol;
    let symbol_token = ncl_object::push_root(&mut ctx, &mut symbol);
    let export_name =
        ncl_object::make_string(&mut ctx, &runtime, &['V', 'I', 'S', 'I', 'B', 'L', 'E'])?;
    Package::from_word(package).export(&mut ctx, &runtime, export_name)?;
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let mut values = MultipleValues::new();
    let all_packages = list_all_packages(&mut ctx, &runtime, &BuiltinArgs::new(&[]), &mut values)?;
    let mut all_packages = all_packages;
    let all_packages_result_token = ncl_object::push_root(&mut ctx, &mut all_packages);
    assert!(list_items(&ctx, all_packages)?.contains(&package));

    let mut name =
        ncl_object::make_string(&mut ctx, &runtime, &['V', 'I', 'S', 'I', 'B', 'L', 'E'])?;
    let name_token = ncl_object::push_root(&mut ctx, &mut name);
    let found = find_all_symbols(&mut ctx, &runtime, &BuiltinArgs::new(&[name]), &mut values)?;
    let mut found = found;
    let found_token = ncl_object::push_root(&mut ctx, &mut found);
    assert_eq!(list_items(&ctx, found)?, vec![symbol]);

    assert!(ncl_object::pop_root(&mut ctx, found_token));
    assert!(ncl_object::pop_root(&mut ctx, name_token));
    assert!(ncl_object::pop_root(&mut ctx, all_packages_result_token));
    assert!(ncl_object::pop_root(&mut ctx, symbol_token));
    assert!(ncl_object::pop_root(&mut ctx, package_token));
    Ok(())
}

#[test]
fn package_mutations_reject_bad_designators_and_preserve_locked_packages() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;

    let package = runtime.ensure_package(&mut ctx, "N25-MUTATION-EDGES")?;
    let rename = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "RENAME-PACKAGE")
            .unwrap_or_else(|| panic!("RENAME-PACKAGE was not registered")),
    )?;
    let new_name = ncl_object::make_string(&mut ctx, &runtime, &['N', '2', '5', '-', 'R'])?;
    let invalid_nickname = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL)?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, rename, &[package, new_name, invalid_nickname],),
        Err(ObjectError::TypeError)
    );

    let delete = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "DELETE-PACKAGE")
            .unwrap_or_else(|| panic!("DELETE-PACKAGE was not registered")),
    )?;
    Package::from_word(package).set_locked(&mut ctx, true)?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, delete, &[package]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn make_package_accepts_metadata_options_and_rejects_unpaired_options() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let make_package = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-PACKAGE")
            .ok_or(ObjectError::Layout)?,
    )?;
    let package_name = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &['N', '2', '5', '-', 'M', 'E', 'T', 'A'],
    )?;
    let documentation = ncl_object::make_string(&mut ctx, &runtime, &['D', 'O', 'C'])?;
    let documentation_key = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &[
            'D', 'O', 'C', 'U', 'M', 'E', 'N', 'T', 'A', 'T', 'I', 'O', 'N',
        ],
    )?;
    let size_key = ncl_object::make_string(&mut ctx, &runtime, &['S', 'I', 'Z', 'E'])?;
    let package = runtime.call_builtin(
        &mut ctx,
        make_package,
        &[
            package_name,
            documentation_key,
            documentation,
            size_key,
            Word::fixnum(32),
        ],
    )?;
    assert!(matches!(
        ncl_object::classify_object(&ctx, package),
        ncl_object::ObjectRef::Package(_)
    ));
    let invalid_name = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &['N', '2', '5', '-', 'U', 'N', 'P', 'A', 'I', 'R', 'E', 'D'],
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, make_package, &[invalid_name, documentation_key]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn package_local_nickname_resolution_survives_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let mut owner = runtime.ensure_package(&mut ctx, "N25-STRESS-PLN-OWNER")?;
    let owner_token = ncl_object::push_root(&mut ctx, &mut owner);
    let mut target = runtime.ensure_package(&mut ctx, "N25-STRESS-PLN-TARGET")?;
    let target_token = ncl_object::push_root(&mut ctx, &mut target);
    let mut nickname = ncl_object::make_string(&mut ctx, &runtime, &['L', 'O', 'C', 'A', 'L'])?;
    let nickname_token = ncl_object::push_root(&mut ctx, &mut nickname);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let mut entry = ncl_object::make_cons(&mut ctx, &runtime, nickname, target)?;
    let entry_token = ncl_object::push_root(&mut ctx, &mut entry);
    let mut entries = ncl_object::make_cons(&mut ctx, &runtime, entry, Word::NIL)?;
    let entries_token = ncl_object::push_root(&mut ctx, &mut entries);
    Package::from_word(owner).set_local_nicknames(&mut ctx, entries)?;
    assert_eq!(
        Package::from_word(owner)
            .resolve_local_nickname(&ctx, StringObject::from_word(nickname),)?,
        Some(Package::from_word(target))
    );

    assert!(ncl_object::pop_root(&mut ctx, entries_token));
    assert!(ncl_object::pop_root(&mut ctx, entry_token));
    assert!(ncl_object::pop_root(&mut ctx, nickname_token));
    assert!(ncl_object::pop_root(&mut ctx, target_token));
    assert!(ncl_object::pop_root(&mut ctx, owner_token));
    Ok(())
}

#[test]
fn package_builtins_cover_designators_and_mutations() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let package = runtime.ensure_package(&mut ctx, "N25-PACKAGE-COVERAGE")?;
    let other = runtime.ensure_package(&mut ctx, "N25-PACKAGE-OTHER")?;
    let package_name = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &[
            'N', '2', '5', '-', 'P', 'A', 'C', 'K', 'A', 'G', 'E', '-', 'C', 'O', 'V', 'E', 'R',
            'A', 'G', 'E',
        ],
    )?;
    assert_eq!(
        call(&runtime, &mut ctx, "FIND-PACKAGE", &[package_name])?,
        package
    );
    assert_eq!(
        call(&runtime, &mut ctx, "FIND-PACKAGE", &[Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        call(&runtime, &mut ctx, "PACKAGEP", &[package])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "PACKAGEP", &[Word::NIL])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "PACKAGE-NICKNAMES", &[package])?,
        Word::NIL
    );
    assert_eq!(
        call(&runtime, &mut ctx, "PACKAGE-SHADOWING-SYMBOLS", &[package])?,
        Word::NIL
    );

    let name = ncl_object::make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E'])?;
    let symbol = call(&runtime, &mut ctx, "INTERN", &[name, package])?;
    let symbols = ncl_object::make_cons(&mut ctx, &runtime, symbol, Word::NIL)?;
    assert_eq!(
        call(&runtime, &mut ctx, "EXPORT", &[symbols, package])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "UNEXPORT", &[symbols, package])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "IMPORT", &[symbols, package])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "UNINTERN", &[name, package])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "UNINTERN", &[name, package])?,
        Word::NIL
    );

    assert_eq!(
        call(&runtime, &mut ctx, "USE-PACKAGE", &[other, package])?,
        Word::TRUE
    );
    let used = call(&runtime, &mut ctx, "PACKAGE-USE-LIST", &[package])?;
    assert_eq!(list_items(&ctx, used)?, vec![other]);
    assert_eq!(
        call(&runtime, &mut ctx, "UNUSE-PACKAGE", &[other, package])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "UNUSE-PACKAGE", &[other, package])?,
        Word::NIL
    );

    let new_name =
        ncl_object::make_string(&mut ctx, &runtime, &['R', 'E', 'N', 'A', 'M', 'E', 'D'])?;
    assert_eq!(
        call(&runtime, &mut ctx, "RENAME-PACKAGE", &[package, new_name])?,
        package
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DELETE-PACKAGE", &[package])?,
        Word::TRUE
    );
    assert_eq!(
        call(&runtime, &mut ctx, "DELETE-PACKAGE", &[package])?,
        Word::NIL
    );
    Ok(())
}

#[test]
fn designator_helpers_cover_symbol_string_and_invalid_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let package = runtime.ensure_package(&mut ctx, "N25-DESIGNATORS")?;
    let package = Package::from_word(package);
    let name = ncl_object::make_string(&mut ctx, &runtime, &['N', 'A', 'M', 'E'])?;
    let package_name = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &[
            'N', '2', '5', '-', 'D', 'E', 'S', 'I', 'G', 'N', 'A', 'T', 'O', 'R', 'S',
        ],
    )?;
    let symbol = package.intern(&mut ctx, &runtime, "NAME")?.0;
    assert_eq!(string_designator(&ctx, name)?.as_word(), name);
    let symbol_name = string_designator(&ctx, symbol)?.as_word();
    assert_eq!(ncl_object::string_length(&ctx, symbol_name)?, 4);
    assert_eq!(
        package_designator(&ctx, &runtime, package.as_word())?,
        package
    );
    assert_eq!(package_designator(&ctx, &runtime, package_name)?, package);
    assert_eq!(
        string_designator(&ctx, Word::fixnum(1)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        package_designator(&ctx, &runtime, Word::fixnum(1)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(list_items(&ctx, Word::NIL)?, Vec::new());
    let expected_status = Package::from_word(
        runtime
            .find_package(&ctx, "KEYWORD")
            .ok_or(ObjectError::PackageConflict)?,
    )
    .intern(&mut ctx, &runtime, "INTERNAL")?
    .0;
    assert_eq!(
        status_word(&mut ctx, &runtime, ncl_object::FindStatus::Internal)?,
        expected_status
    );
    Ok(())
}

use super::*;
use ncl_object::FunctionObject;

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
fn intern_uses_current_package_when_package_is_omitted() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let package = runtime.ensure_package(&mut ctx, "N25-INTERN-DEFAULT")?;
    let common = Package::from_word(
        runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?,
    );
    let (current_package, _) = common.intern(&mut ctx, &runtime, "*PACKAGE*")?;
    let previous_package = ncl_object::symbol_value(&ctx, current_package)?;
    ncl_object::set_symbol_value(&mut ctx, current_package, package)?;

    let function = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "INTERN")
            .ok_or(ObjectError::Layout)?,
    )?;
    let name = ncl_object::make_string(&mut ctx, &runtime, &['D', 'E', 'F', 'A', 'U', 'L', 'T'])?;
    let symbol = runtime.call_builtin(&mut ctx, function, &[name])?;

    assert_eq!(ncl_object::symbol_package(&ctx, symbol)?, package);
    ncl_object::set_symbol_value(&mut ctx, current_package, previous_package)?;
    Ok(())
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
fn use_and_unuse_package_accept_package_list_designators() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let function = |ctx: &mut ThreadContext, name: &str| {
        FunctionObject::try_from(
            runtime
                .function(ctx, "COMMON-LISP", name)
                .ok_or(ObjectError::Layout)?,
        )
    };

    let by_package = runtime.ensure_package(&mut ctx, "N25-USE-LIST-PACKAGE")?;
    let by_string = runtime.ensure_package(&mut ctx, "N25-USE-LIST-STRING")?;
    let by_symbol = runtime.ensure_package(&mut ctx, "N25-USE-LIST-SYMBOL")?;
    let target = runtime.ensure_package(&mut ctx, "N25-USE-LIST-TARGET")?;
    let string_designator = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &"N25-USE-LIST-STRING".chars().collect::<Vec<_>>(),
    )?;
    let common = Package::from_word(
        runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?,
    );
    let symbol_designator = common.intern(&mut ctx, &runtime, "N25-USE-LIST-SYMBOL")?.0;
    let tail = ncl_object::make_cons(&mut ctx, &runtime, symbol_designator, Word::NIL)?;
    let tail = ncl_object::make_cons(&mut ctx, &runtime, string_designator, tail)?;
    let designators = ncl_object::make_cons(&mut ctx, &runtime, by_package, tail)?;
    let use_package = function(&mut ctx, "USE-PACKAGE")?;
    let unuse_package = function(&mut ctx, "UNUSE-PACKAGE")?;
    let package_use_list = function(&mut ctx, "PACKAGE-USE-LIST")?;

    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[designators, target]),
        Ok(Word::TRUE)
    );
    let used = runtime.call_builtin(&mut ctx, package_use_list, &[target])?;
    let used = list_items(&ctx, used)?;
    assert_eq!(used.len(), 3);
    assert!(used.contains(&by_package));
    assert!(used.contains(&by_string));
    assert!(used.contains(&by_symbol));

    assert_eq!(
        runtime.call_builtin(&mut ctx, unuse_package, &[designators, target]),
        Ok(Word::TRUE)
    );
    let used = runtime.call_builtin(&mut ctx, package_use_list, &[target])?;
    assert!(list_items(&ctx, used)?.is_empty());
    assert_eq!(
        runtime.call_builtin(&mut ctx, unuse_package, &[designators, target]),
        Ok(Word::NIL)
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[by_string, target]),
        Ok(Word::TRUE)
    );
    let used = runtime.call_builtin(&mut ctx, package_use_list, &[target])?;
    assert_eq!(list_items(&ctx, used)?, vec![by_string]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, unuse_package, &[by_string, target]),
        Ok(Word::TRUE)
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[Word::NIL, target]),
        Ok(Word::TRUE)
    );
    let invalid_tail = ncl_object::make_cons(&mut ctx, &runtime, Word::fixnum(0), Word::NIL)?;
    let invalid_designators = ncl_object::make_cons(&mut ctx, &runtime, by_package, invalid_tail)?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[invalid_designators, target]),
        Err(ObjectError::TypeError)
    );
    let used = runtime.call_builtin(&mut ctx, package_use_list, &[target])?;
    assert!(list_items(&ctx, used)?.is_empty());

    let circular = ncl_object::make_cons(&mut ctx, &runtime, by_package, Word::NIL)?;
    ncl_object::rplacd(&mut ctx, circular, circular)?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[circular, target]),
        Err(ObjectError::TypeError)
    );
    let used = runtime.call_builtin(&mut ctx, package_use_list, &[target])?;
    assert!(list_items(&ctx, used)?.is_empty());

    Ok(())
}

#[test]
fn use_package_rejects_improper_designator_list_before_mutation() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let source = runtime.ensure_package(&mut ctx, "N25-IMPROPER-USE-SOURCE")?;
    let target = runtime.ensure_package(&mut ctx, "N25-IMPROPER-USE-TARGET")?;
    let improper = ncl_object::make_cons(&mut ctx, &runtime, source, Word::fixnum(25))?;
    let use_package = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "USE-PACKAGE")
            .ok_or(ObjectError::Layout)?,
    )?;
    let package_use_list = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "PACKAGE-USE-LIST")
            .ok_or(ObjectError::Layout)?,
    )?;

    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[improper, target]),
        Err(ObjectError::TypeError)
    );
    let used = runtime.call_builtin(&mut ctx, package_use_list, &[target])?;
    assert!(list_items(&ctx, used)?.is_empty());
    Ok(())
}

#[test]
fn use_package_uses_omitted_current_package_under_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let source = runtime.ensure_package(&mut ctx, "N25-GC-USE-PACKAGE")?;
    runtime.ensure_package(&mut ctx, "N25-GC-USE-STRING")?;
    runtime.ensure_package(&mut ctx, "N25-GC-USE-SYMBOL")?;
    let mut target = runtime.ensure_package(&mut ctx, "N25-GC-USE-TARGET")?;
    let target_token = ncl_object::push_root(&mut ctx, &mut target);
    let common = Package::from_word(
        runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?,
    );
    let mut package_variable = common.intern(&mut ctx, &runtime, "*PACKAGE*")?.0;
    let package_variable_token = ncl_object::push_root(&mut ctx, &mut package_variable);
    ncl_object::set_symbol_value(&mut ctx, package_variable, target)?;

    let string_designator = ncl_object::make_string(
        &mut ctx,
        &runtime,
        &"N25-GC-USE-STRING".chars().collect::<Vec<_>>(),
    )?;
    let symbol_designator = common.intern(&mut ctx, &runtime, "N25-GC-USE-SYMBOL")?.0;
    let tail = ncl_object::make_cons(&mut ctx, &runtime, symbol_designator, Word::NIL)?;
    let tail = ncl_object::make_cons(&mut ctx, &runtime, string_designator, tail)?;
    let designators = ncl_object::make_cons(&mut ctx, &runtime, source, tail)?;
    let use_package = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "USE-PACKAGE")
            .ok_or(ObjectError::Layout)?,
    )?;

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[designators]),
        Ok(Word::TRUE)
    );
    let used = Package::from_word(target).use_list(&ctx)?;
    let used = list_items(&ctx, used)?;
    assert_eq!(used.len(), 3);
    for name in [
        "N25-GC-USE-PACKAGE",
        "N25-GC-USE-STRING",
        "N25-GC-USE-SYMBOL",
    ] {
        let expected = runtime
            .find_package(&ctx, name)
            .ok_or(ObjectError::Layout)?;
        assert!(used.contains(&expected));
    }

    assert!(ncl_object::pop_root(&mut ctx, package_variable_token));
    assert!(ncl_object::pop_root(&mut ctx, target_token));
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
            .resolve_local_nickname(&ctx, ncl_object::StringObject::from_word(nickname),)?,
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
fn package_and_string_designators_accept_characters() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let package = runtime.ensure_package(&mut ctx, "P")?;
    let target = runtime.ensure_package(&mut ctx, "T")?;
    let function = |ctx: &mut ThreadContext, name: &str| {
        FunctionObject::try_from(
            runtime
                .function(ctx, "COMMON-LISP", name)
                .ok_or(ObjectError::Layout)?,
        )
    };
    let package_char = Word::character(u32::from('P'));
    let target_char = Word::character(u32::from('T'));

    let find_package = function(&mut ctx, "FIND-PACKAGE")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[package_char]),
        Ok(package)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[package]),
        Ok(package)
    );

    let package_name = function(&mut ctx, "PACKAGE-NAME")?;
    let expected_package_name = Package::from_word(package).name(&ctx)?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, package_name, &[package_char]),
        Ok(expected_package_name)
    );
    let package_nicknames = function(&mut ctx, "PACKAGE-NICKNAMES")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, package_nicknames, &[package_char]),
        Ok(Word::NIL)
    );
    let package_shadowing_symbols = function(&mut ctx, "PACKAGE-SHADOWING-SYMBOLS")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, package_shadowing_symbols, &[package_char]),
        Ok(Word::NIL)
    );

    let use_package = function(&mut ctx, "USE-PACKAGE")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, use_package, &[package_char, target_char]),
        Ok(Word::TRUE)
    );
    let package_use_list = function(&mut ctx, "PACKAGE-USE-LIST")?;
    let used_by_target = runtime.call_builtin(&mut ctx, package_use_list, &[target_char])?;
    assert_eq!(list_items(&ctx, used_by_target)?, vec![package]);
    let package_used_by_list = function(&mut ctx, "PACKAGE-USED-BY-LIST")?;
    let users_of_package = runtime.call_builtin(&mut ctx, package_used_by_list, &[package_char])?;
    assert_eq!(list_items(&ctx, users_of_package)?, vec![target]);

    let intern = function(&mut ctx, "INTERN")?;
    let character_name = Word::character(u32::from('Z'));
    let string_name = ncl_object::make_string(&mut ctx, &runtime, &['Z'])?;
    let symbol = runtime.call_builtin(&mut ctx, intern, &[string_name, package_char])?;
    let find_symbol = function(&mut ctx, "FIND-SYMBOL")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_symbol, &[string_name, package_char]),
        Ok(symbol)
    );
    let find_all_symbols = function(&mut ctx, "FIND-ALL-SYMBOLS")?;
    let all_matching_symbols =
        runtime.call_builtin(&mut ctx, find_all_symbols, &[character_name])?;
    assert!(list_items(&ctx, all_matching_symbols)?.contains(&symbol));
    let unintern = function(&mut ctx, "UNINTERN")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, unintern, &[symbol, package_char]),
        Ok(Word::TRUE)
    );

    let shadow = function(&mut ctx, "SHADOW")?;
    let shadow_name = ncl_object::make_cons(
        &mut ctx,
        &runtime,
        Word::character(u32::from('X')),
        Word::NIL,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, shadow, &[shadow_name, package_char],),
        Ok(Word::TRUE)
    );
    let shadowing = runtime.call_builtin(&mut ctx, package_shadowing_symbols, &[package_char])?;
    let shadowing = list_items(&ctx, shadowing)?;
    assert_eq!(shadowing.len(), 1);
    let shadow_name = ncl_object::symbol_name(&ctx, shadowing[0])?;
    assert_eq!(ncl_object::string_length(&ctx, shadow_name)?, 1);
    assert_eq!(ncl_object::string_ref(&ctx, shadow_name, 0)?, 'X');

    let delete_package = function(&mut ctx, "DELETE-PACKAGE")?;
    runtime.ensure_package(&mut ctx, "D")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, delete_package, &[Word::character(u32::from('D'))],),
        Ok(Word::TRUE)
    );
    Ok(())
}

#[test]
fn make_and_rename_package_accept_character_designators() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let function = |ctx: &mut ThreadContext, name: &str| {
        FunctionObject::try_from(
            runtime
                .function(ctx, "COMMON-LISP", name)
                .ok_or(ObjectError::Layout)?,
        )
    };
    let make_package = function(&mut ctx, "MAKE-PACKAGE")?;
    let nicknames_key =
        ncl_object::make_string(&mut ctx, &runtime, &"NICKNAMES".chars().collect::<Vec<_>>())?;
    let nickname = Word::character(u32::from('R'));
    let nickname_list = ncl_object::make_cons(&mut ctx, &runtime, nickname, Word::NIL)?;
    let package = runtime.call_builtin(
        &mut ctx,
        make_package,
        &[
            Word::character(u32::from('Q')),
            nicknames_key,
            nickname_list,
        ],
    )?;
    let find_package = function(&mut ctx, "FIND-PACKAGE")?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[Word::character(u32::from('Q'))]),
        Ok(package)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[nickname]),
        Ok(package)
    );
    let package_nicknames = function(&mut ctx, "PACKAGE-NICKNAMES")?;
    let nicknames = runtime.call_builtin(
        &mut ctx,
        package_nicknames,
        &[Word::character(u32::from('Q'))],
    )?;
    let nicknames = list_items(&ctx, nicknames)?;
    assert_eq!(nicknames.len(), 1);
    assert_eq!(ncl_object::string_ref(&ctx, nicknames[0], 0)?, 'R');

    let rename_package = function(&mut ctx, "RENAME-PACKAGE")?;
    let renamed_nickname = Word::character(u32::from('S'));
    let renamed_nicknames = ncl_object::make_cons(&mut ctx, &runtime, renamed_nickname, Word::NIL)?;
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            rename_package,
            &[package, Word::character(u32::from('U')), renamed_nicknames,],
        ),
        Ok(package)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[Word::character(u32::from('U'))]),
        Ok(package)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[renamed_nickname]),
        Ok(package)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, find_package, &[Word::character(u32::from('Q'))]),
        Ok(Word::NIL)
    );

    let use_source = runtime.ensure_package(&mut ctx, "V")?;
    let use_key = ncl_object::make_string(&mut ctx, &runtime, &['U', 'S', 'E'])?;
    let use_designator = ncl_object::make_cons(
        &mut ctx,
        &runtime,
        Word::character(u32::from('V')),
        Word::NIL,
    )?;
    let used_package = runtime.call_builtin(
        &mut ctx,
        make_package,
        &[Word::character(u32::from('W')), use_key, use_designator],
    )?;
    let package_use_list = function(&mut ctx, "PACKAGE-USE-LIST")?;
    let used_packages = runtime.call_builtin(
        &mut ctx,
        package_use_list,
        &[Word::character(u32::from('W'))],
    )?;
    assert_eq!(list_items(&ctx, used_packages)?, vec![use_source]);
    let used_package_name = Package::from_word(used_package).name(&ctx)?;
    assert_eq!(ncl_object::string_length(&ctx, used_package_name)?, 1);
    assert_eq!(ncl_object::string_ref(&ctx, used_package_name, 0)?, 'W');
    Ok(())
}

#[test]
fn gentemp_requires_string_prefix_and_accepts_package_designators_under_gc_stress()
-> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let mut package = runtime.ensure_package(&mut ctx, "G")?;
    let package_token = ncl_object::push_root(&mut ctx, &mut package);
    let gentemp = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GENTEMP")
            .ok_or(ObjectError::Layout)?,
    )?;
    let mut string_prefix = ncl_object::make_string(&mut ctx, &runtime, &['S'])?;
    let string_prefix_token = ncl_object::push_root(&mut ctx, &mut string_prefix);

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    assert_eq!(
        runtime.call_builtin(&mut ctx, gentemp, &[Word::character(u32::from('P'))]),
        Err(ObjectError::TypeError)
    );

    let mut character_package_symbol = runtime
        .call_builtin(
            &mut ctx,
            gentemp,
            &[string_prefix, Word::character(u32::from('G'))],
        )
        .unwrap_or_else(|error| panic!("GENTEMP String prefix with Character package: {error:?}"));
    let character_package_symbol_token =
        ncl_object::push_root(&mut ctx, &mut character_package_symbol);
    assert_eq!(
        ncl_object::symbol_package(&ctx, character_package_symbol)?,
        package
    );
    let character_package_name = ncl_object::symbol_name(&ctx, character_package_symbol)?;
    assert_eq!(
        ncl_object::string_ref(&ctx, character_package_name, 0)?,
        'S'
    );
    assert!(ncl_object::pop_root(
        &mut ctx,
        character_package_symbol_token
    ));

    let mut object_package_symbol = runtime
        .call_builtin(&mut ctx, gentemp, &[string_prefix, package])
        .unwrap_or_else(|error| panic!("GENTEMP String prefix with Package object: {error:?}"));
    let object_package_symbol_token = ncl_object::push_root(&mut ctx, &mut object_package_symbol);
    assert_eq!(
        ncl_object::symbol_package(&ctx, object_package_symbol)?,
        package
    );
    let object_package_name = ncl_object::symbol_name(&ctx, object_package_symbol)?;
    assert_eq!(ncl_object::string_ref(&ctx, object_package_name, 0)?, 'S');
    assert!(ncl_object::pop_root(&mut ctx, object_package_symbol_token));

    assert!(ncl_object::pop_root(&mut ctx, string_prefix_token));
    assert!(ncl_object::pop_root(&mut ctx, package_token));
    Ok(())
}

#[test]
fn package_local_nickname_character_designators_survive_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let mut owner = runtime.ensure_package(&mut ctx, "O")?;
    let owner_token = ncl_object::push_root(&mut ctx, &mut owner);
    let mut target = runtime.ensure_package(&mut ctx, "T")?;
    let target_token = ncl_object::push_root(&mut ctx, &mut target);
    let function = |ctx: &mut ThreadContext, name: &str| {
        FunctionObject::try_from(
            runtime
                .function(ctx, "NCL-EXT", name)
                .ok_or(ObjectError::Layout)?,
        )
    };
    let add = function(&mut ctx, "ADD-PACKAGE-LOCAL-NICKNAME")?;
    let remove = function(&mut ctx, "REMOVE-PACKAGE-LOCAL-NICKNAME")?;
    let list = function(&mut ctx, "PACKAGE-LOCAL-NICKNAMES")?;
    let owner_designator = Word::character(u32::from('O'));
    let nickname_designator = Word::character(u32::from('L'));
    let target_designator = Word::character(u32::from('T'));

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let mut added = runtime
        .call_builtin(
            &mut ctx,
            add,
            &[nickname_designator, target_designator, owner_designator],
        )
        .unwrap_or_else(|error| panic!("ADD-PACKAGE-LOCAL-NICKNAME call: {error:?}"));
    let added_token = ncl_object::push_root(&mut ctx, &mut added);
    assert_eq!(
        ncl_object::string_ref(&ctx, added, 0)
            .unwrap_or_else(|error| panic!("added nickname character: {error:?}")),
        'L'
    );
    let entries = runtime
        .call_builtin(&mut ctx, list, &[owner_designator])
        .unwrap_or_else(|error| panic!("PACKAGE-LOCAL-NICKNAMES call: {error:?}"));
    let entry = ncl_object::car(&ctx, entries)
        .unwrap_or_else(|error| panic!("local nickname entry: {error:?}"));
    let stored_nickname = ncl_object::car(&ctx, entry)
        .unwrap_or_else(|error| panic!("stored local nickname: {error:?}"));
    assert_eq!(
        ncl_object::string_ref(&ctx, stored_nickname, 0)
            .unwrap_or_else(|error| panic!("stored nickname character: {error:?}")),
        'L'
    );
    let stored_target = ncl_object::cdr(&ctx, entry)
        .unwrap_or_else(|error| panic!("stored nickname target: {error:?}"));
    assert_eq!(stored_target, target);
    assert_eq!(
        Package::from_word(owner)
            .resolve_local_nickname(&ctx, ncl_object::StringObject::from_word(added),)?,
        Some(Package::from_word(target))
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, remove, &[nickname_designator, owner_designator],),
        Ok(Word::TRUE)
    );
    assert_eq!(
        Package::from_word(owner)
            .resolve_local_nickname(&ctx, ncl_object::StringObject::from_word(added),)?,
        None
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, list, &[owner_designator]),
        Ok(Word::NIL)
    );

    assert!(ncl_object::pop_root(&mut ctx, added_token));
    assert!(ncl_object::pop_root(&mut ctx, target_token));
    assert!(ncl_object::pop_root(&mut ctx, owner_token));
    Ok(())
}

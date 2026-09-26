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
    assert_eq!(list_items(&mut ctx, use_list)?, vec![right.as_word()]);
    let used_by_function = function(&runtime, &mut ctx, "PACKAGE-USED-BY-LIST")?;
    let used_by = runtime.call_builtin(&mut ctx, used_by_function, &[right.as_word()])?;
    assert_eq!(list_items(&mut ctx, used_by)?, vec![left.as_word()]);

    let all_function = function(&runtime, &mut ctx, "LIST-ALL-PACKAGES")?;
    let all = runtime.call_builtin(&mut ctx, all_function, &[])?;
    assert!(list_items(&mut ctx, all)?.contains(&left.as_word()));
    let name = ncl_object::make_string(&mut ctx, &runtime, &['S', 'H', 'A', 'R', 'E', 'D'])?;
    let found_function = function(&runtime, &mut ctx, "FIND-ALL-SYMBOLS")?;
    let found = runtime.call_builtin(&mut ctx, found_function, &[name])?;
    assert_eq!(list_items(&mut ctx, found)?, vec![symbol]);
    Ok(())
}

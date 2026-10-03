use ncl_object::{FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_string, string_length, string_ref};

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .ok_or(ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, function, args)
}

fn string_value(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, word)?).map(|index| string_ref(ctx, word, index)).collect()
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    let package = runtime.find_package(ctx, "KEYWORD").ok_or(ObjectError::PackageConflict)?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

#[test]
fn pathname_builtins_construct_access_and_round_trip() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;

    let name = make_string(&mut ctx, &runtime, &['m', 'a', 'i', 'n'])?;
    let type_ = make_string(&mut ctx, &runtime, &['l', 'i', 's', 'p'])?;
    let directory = Word::NIL;
    let name_key = keyword(&mut ctx, &runtime, "NAME")?;
    let type_key = keyword(&mut ctx, &runtime, "TYPE")?;
    let directory_key = keyword(&mut ctx, &runtime, "DIRECTORY")?;
    let pathname = call(
        &runtime,
        &mut ctx,
        "MAKE-PATHNAME",
        &[
            name_key, name,
            type_key, type_,
            directory_key, directory,
        ],
    )?;
    assert_eq!(call(&runtime, &mut ctx, "PATHNAMEP", &[pathname])?, Word::TRUE);
    assert_eq!(call(&runtime, &mut ctx, "PATHNAME-NAME", &[pathname])?, name);
    assert_eq!(call(&runtime, &mut ctx, "PATHNAME-TYPE", &[pathname])?, type_);
    let namestring = call(&runtime, &mut ctx, "NAMESTRING", &[pathname])?;
    assert_eq!(string_value(&ctx, namestring)?, "main.lisp");
    let parsed = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[namestring])?;
    assert_eq!(ctx.values().len(), 3);
    let reparsed_namestring = call(&runtime, &mut ctx, "NAMESTRING", &[parsed])?;
    assert_eq!(string_value(&ctx, reparsed_namestring)?, "main.lisp");
    Ok(())
}

#[test]
fn parse_namestring_preserves_absolute_directory() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let input = make_string(&mut ctx, &runtime, &['/', 't', 'm', 'p', '/', 'x', '.', 't', 'x', 't'])?;
    let pathname = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[input])?;
    let namestring = call(&runtime, &mut ctx, "NAMESTRING", &[pathname])?;
    assert_eq!(string_value(&ctx, namestring)?, "/tmp/x.txt");
    Ok(())
}

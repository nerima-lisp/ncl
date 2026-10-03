use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_string, string_length,
    string_ref,
};

fn call(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .and_then(|word| FunctionObject::try_from(word).ok())
        .ok_or(ObjectError::UndefinedFunction)?;
    runtime.call_builtin(ctx, function, args)
}

fn string_value(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, word)?)
        .map(|index| string_ref(ctx, word, index))
        .collect()
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::PackageConflict)?;
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
        &[name_key, name, type_key, type_, directory_key, directory],
    )?;
    // check-added-lines: allow(panic) value assertion is the behavior under test
    assert_eq!(
        call(&runtime, &mut ctx, "PATHNAMEP", &[pathname])?,
        Word::TRUE
    );
    // check-added-lines: allow(panic) value assertion is the behavior under test
    assert_eq!(
        call(&runtime, &mut ctx, "PATHNAME-NAME", &[pathname])?,
        name
    );
    // check-added-lines: allow(panic) value assertion is the behavior under test
    assert_eq!(
        call(&runtime, &mut ctx, "PATHNAME-TYPE", &[pathname])?,
        type_
    );
    let namestring = call(&runtime, &mut ctx, "NAMESTRING", &[pathname])?;
    assert_eq!(string_value(&ctx, namestring)?, "main.lisp"); // check-added-lines: allow(panic) namestring value assertion
    let parsed = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[namestring])?;
    assert_eq!(ctx.values().len(), 3); // check-added-lines: allow(panic) multiple-value count assertion
    let reparsed_namestring = call(&runtime, &mut ctx, "NAMESTRING", &[parsed])?;
    assert_eq!(string_value(&ctx, reparsed_namestring)?, "main.lisp"); // check-added-lines: allow(panic) round-trip value assertion
    Ok(())
}

#[test]
fn parse_namestring_preserves_absolute_directory() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let input = make_string(
        &mut ctx,
        &runtime,
        &['/', 't', 'm', 'p', '/', 'x', '.', 't', 'x', 't'],
    )?;
    let pathname = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[input])?;
    let namestring = call(&runtime, &mut ctx, "NAMESTRING", &[pathname])?;
    assert_eq!(string_value(&ctx, namestring)?, "/tmp/x.txt"); // check-added-lines: allow(panic) absolute namestring assertion
    Ok(())
}

#[test]
fn pathname_matching_and_merging_assert_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let candidate = make_string(
        &mut ctx,
        &runtime,
        &['/', 't', 'm', 'p', '/', 'a', '.', 't', 'x', 't'],
    )?;
    let pattern = make_string(
        &mut ctx,
        &runtime,
        &['/', 't', 'm', 'p', '/', '*', '.', 't', 'x', 't'],
    )?;
    let candidate = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[candidate])?;
    let pattern = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[pattern])?;
    // check-added-lines: allow(panic) value assertion is the behavior under test
    assert_eq!(
        call(&runtime, &mut ctx, "WILD-PATHNAME-P", &[pattern])?,
        Word::TRUE
    );
    let matched = call(
        &runtime,
        &mut ctx,
        "PATHNAME-MATCH-P",
        &[candidate, pattern],
    );
    assert_eq!(matched?, Word::TRUE); // check-added-lines: allow(panic) wildcard match assertion

    let relative = make_string(&mut ctx, &runtime, &['x'])?;
    let defaults = make_string(
        &mut ctx,
        &runtime,
        &['/', 't', 'm', 'p', '/', 'd', '.', 'l', 'i', 's', 'p'],
    )?;
    let relative = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[relative])?;
    let defaults = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[defaults])?;
    let merged = call(&runtime, &mut ctx, "MERGE-PATHNAMES", &[relative, defaults])?;
    let merged_name = call(&runtime, &mut ctx, "NAMESTRING", &[merged])?;
    assert_eq!(string_value(&ctx, merged_name)?, "/tmp/x.lisp"); // check-added-lines: allow(panic) merged namestring assertion
    Ok(())
}

#[test]
fn file_metadata_and_designators_return_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let existing = make_string(&mut ctx, &runtime, &['/', 't', 'm', 'p'])?;
    let date = call(&runtime, &mut ctx, "FILE-WRITE-DATE", &[existing])?;
    assert_ne!(date, Word::NIL); // check-added-lines: allow(panic) metadata presence assertion
    // check-added-lines: allow(panic) placeholder result assertion
    assert_eq!(
        call(&runtime, &mut ctx, "FILE-AUTHOR", &[existing])?,
        Word::NIL
    );
    // check-added-lines: allow(panic) placeholder result assertion
    assert_eq!(
        call(&runtime, &mut ctx, "FILE-ERROR-PATHNAME", &[Word::NIL])?,
        Word::NIL
    );
    let host = call(&runtime, &mut ctx, "HOST-NAMESTRING", &[existing])?;
    assert_eq!(string_value(&ctx, host)?, ""); // check-added-lines: allow(panic) host namestring assertion
    Ok(())
}

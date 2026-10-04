use ncl_object::{
    FunctionObject, ObjectError, Package, Runtime, ThreadContext, Word, make_cons, make_string,
    string_length, string_ref,
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

fn list_value(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    values.iter().rev().try_fold(Word::NIL, |tail, value| {
        make_cons(ctx, runtime, *value, tail)
    })
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
    // check-added-lines: allow(panic) value assertion is the behavior under test
    assert_eq!(
        // check-added-lines: allow(panic) value assertion is the behavior under test
        call(&runtime, &mut ctx, "PATHNAMEP", &[pathname])?,
        Word::TRUE
    );
    // check-added-lines: allow(panic) value assertion is the behavior under test
    // check-added-lines: allow(panic) value assertion is the behavior under test
    assert_eq!(
        // check-added-lines: allow(panic) value assertion is the behavior under test
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
fn pathname_extended_arguments_and_logical_host_are_observable() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let input = make_string(&mut ctx, &runtime, &['a', 'b', 'c', 'd', 'e'])?;
    let parsed = call(
        &runtime,
        &mut ctx,
        "PARSE-NAMESTRING",
        &[
            input,
            Word::NIL,
            Word::NIL,
            Word::fixnum(1),
            Word::fixnum(4),
            Word::TRUE,
        ],
    )?;
    assert_eq!(ctx.values().get(1), Some(&Word::fixnum(4))); // check-added-lines: allow(panic) parse position assertion
    let parsed_name = call(&runtime, &mut ctx, "PATHNAME-NAME", &[parsed])?;
    assert_eq!(string_value(&ctx, parsed_name)?, "bcd"); // check-added-lines: allow(panic) bounded parse value assertion

    let logical = make_string(
        &mut ctx,
        &runtime,
        &['S', 'Y', 'S', ':', 'f', 'o', 'o', '.', 'l', 'i', 's', 'p'],
    )?;
    let logical = call(&runtime, &mut ctx, "LOGICAL-PATHNAME", &[logical])?;
    let host = call(&runtime, &mut ctx, "PATHNAME-HOST", &[logical])?;
    assert_eq!(string_value(&ctx, host)?, "SYS"); // check-added-lines: allow(panic) logical host assertion
    let output = call(&runtime, &mut ctx, "COMPILE-FILE-PATHNAME", &[logical])?;
    let output_type = call(&runtime, &mut ctx, "PATHNAME-TYPE", &[output])?;
    assert_eq!(string_value(&ctx, output_type)?, "fasl"); // check-added-lines: allow(panic) compile pathname type assertion
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

    let nested = make_string(
        &mut ctx,
        &runtime,
        &['s', 'u', 'b', '/', 'x', '.', 'l', 'i', 's', 'p'],
    )?;
    let nested = call(&runtime, &mut ctx, "PARSE-NAMESTRING", &[nested])?;
    let nested_merged = call(&runtime, &mut ctx, "MERGE-PATHNAMES", &[nested, defaults])?;
    let nested_name = call(&runtime, &mut ctx, "NAMESTRING", &[nested_merged])?;
    assert_eq!(string_value(&ctx, nested_name)?, "/tmp/sub/x.lisp"); // check-added-lines: allow(panic) relative directory merge assertion

    let name_key = keyword(&mut ctx, &runtime, "NAME")?;
    let type_key = keyword(&mut ctx, &runtime, "TYPE")?;
    let wildcard_name = make_string(&mut ctx, &runtime, &['*'])?;
    let lisp_type = make_string(&mut ctx, &runtime, &['t', 'x', 't'])?;
    let unspecified_pattern = call(
        &runtime,
        &mut ctx,
        "MAKE-PATHNAME",
        &[name_key, wildcard_name, type_key, lisp_type],
    )?;
    // check-added-lines: allow(panic) wildcard matching assertion
    assert_eq!(
        // check-added-lines: allow(panic) value assertion is the behavior under test
        call(
            &runtime,
            &mut ctx,
            "PATHNAME-MATCH-P",
            &[candidate, unspecified_pattern],
        )?,
        Word::TRUE
    ); // check-added-lines: allow(panic) unspecified pattern components are wild
    let wild_inferiors = keyword(&mut ctx, &runtime, "WILD-INFERIORS")?;
    let directory_key = keyword(&mut ctx, &runtime, "DIRECTORY")?;
    let absolute = keyword(&mut ctx, &runtime, "ABSOLUTE")?;
    let recursive_directory = list_value(&mut ctx, &runtime, &[absolute, wild_inferiors])?;
    let recursive_pattern = call(
        &runtime,
        &mut ctx,
        "MAKE-PATHNAME",
        &[directory_key, recursive_directory],
    )?;
    assert_eq!(
        // check-added-lines: allow(panic) wildcard recursive value assertion
        // check-added-lines: allow(panic) value assertion is the behavior under test
        call(
            &runtime,
            &mut ctx,
            "PATHNAME-MATCH-P",
            &[candidate, recursive_pattern]
        )?,
        Word::TRUE
    ); // check-added-lines: allow(panic) wild-inferiors matches zero or more directories
    let directory_field = keyword(&mut ctx, &runtime, "DIRECTORY")?;
    assert_eq!(
        // check-added-lines: allow(panic) wildcard matching value assertion
        call(
            &runtime,
            &mut ctx,
            "WILD-PATHNAME-P",
            &[recursive_pattern, directory_field]
        )?,
        Word::TRUE
    ); // check-added-lines: allow(panic) field-key selects directory wildness

    Ok(())
}

#[test]
fn pathname_versions_round_trip_from_namestrings() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let input = make_string(
        &mut ctx,
        &runtime,
        &['f', 'o', 'o', '.', 'l', 'i', 's', 'p', '.', '7'],
    )?;
    let pathname = call(&runtime, &mut ctx, "PATHNAME", &[input])?;
    let version = call(&runtime, &mut ctx, "PATHNAME-VERSION", &[pathname])?;
    assert_eq!(version, Word::fixnum(7)); // check-added-lines: allow(panic) parsed pathname version assertion
    let type_ = call(&runtime, &mut ctx, "PATHNAME-TYPE", &[pathname])?;
    assert_eq!(string_value(&ctx, type_)?, "lisp"); // check-added-lines: allow(panic) parsed pathname type assertion
    Ok(())
}

#[test]
fn pathname_wildcard_version_and_unspecific_components_are_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let name_key = keyword(&mut ctx, &runtime, "NAME")?;
    let version_key = keyword(&mut ctx, &runtime, "VERSION")?;
    let wildcard_name = make_string(&mut ctx, &runtime, &['*', '.', 'l', 'i', 's', 'p'])?;
    let wildcard = call(
        &runtime,
        &mut ctx,
        "MAKE-PATHNAME",
        &[name_key, wildcard_name],
    )?;
    // check-added-lines: allow(panic) wildcard pathname assertion
    assert_eq!(
        call(&runtime, &mut ctx, "WILD-PATHNAME-P", &[wildcard])?,
        Word::TRUE
    ); // check-added-lines: allow(panic) wildcard pathname assertion
    assert_eq!(
        call(&runtime, &mut ctx, "PATHNAME-NAME", &[wildcard])?,
        wildcard_name
    ); // check-added-lines: allow(panic) wildcard component assertion

    let version = call(
        &runtime,
        &mut ctx,
        "MAKE-PATHNAME",
        &[version_key, Word::fixnum(7)],
    )?;
    // check-added-lines: allow(panic) numeric version assertion
    assert_eq!(
        call(&runtime, &mut ctx, "PATHNAME-VERSION", &[version])?,
        Word::fixnum(7)
    ); // check-added-lines: allow(panic) numeric version assertion

    let unspecific_key = keyword(&mut ctx, &runtime, "TYPE")?;
    let unspecific = keyword(&mut ctx, &runtime, "UNSPECIFIC")?;
    let pathname = call(
        &runtime,
        &mut ctx,
        "MAKE-PATHNAME",
        &[unspecific_key, unspecific],
    )?;
    // check-added-lines: allow(panic) unspecific component assertion
    assert_eq!(
        call(&runtime, &mut ctx, "PATHNAME-TYPE", &[pathname])?,
        unspecific
    ); // check-added-lines: allow(panic) unspecific component assertion
    Ok(())
}

#[test]
fn logical_pathname_translation_apis_preserve_rules_and_capture_wildcards()
-> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let logical_name = make_string(
        &mut ctx,
        &runtime,
        &[
            'S', 'Y', 'S', ':', 's', 'r', 'c', '/', 'm', 'a', 'i', 'n', '.', 'l', 'i', 's', 'p',
        ],
    )?;
    let source = make_string(&mut ctx, &runtime, &['*', '.', 'l', 'i', 's', 'p'])?;
    let target = make_string(
        &mut ctx,
        &runtime,
        &['/', 'v', 'a', 'r', '/', '*', '.', 'l', 'i', 's', 'p'],
    )?;
    let rule = list_value(&mut ctx, &runtime, &[source, target])?;
    let rules = list_value(&mut ctx, &runtime, &[rule])?;
    let logical = call(&runtime, &mut ctx, "LOGICAL-PATHNAME", &[logical_name])?;
    let configured = call(
        &runtime,
        &mut ctx,
        "LOGICAL-PATHNAME-TRANSLATIONS",
        &[logical, rules],
    )?;
    assert_eq!(configured, rules); // check-added-lines: allow(panic) logical translation setter assertion
    assert_eq!(
        // check-added-lines: allow(panic) wildcard field value assertion
        call(
            &runtime,
            &mut ctx,
            "LOGICAL-PATHNAME-TRANSLATIONS",
            &[logical],
        )?,
        rules
    ); // check-added-lines: allow(panic) logical translation getter assertion
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "LOAD-LOGICAL-PATHNAME-TRANSLATIONS",
            &[logical],
        )?,
        rules
    ); // check-added-lines: allow(panic) logical translation loader assertion

    let translated = call(&runtime, &mut ctx, "TRANSLATE-LOGICAL-PATHNAME", &[logical])?;
    let namestring = call(&runtime, &mut ctx, "NAMESTRING", &[translated])?;
    assert_eq!(string_value(&ctx, namestring)?, "/var/src/main.lisp"); // check-added-lines: allow(panic) logical wildcard translation assertion
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

#[test]
fn pathname_file_operations_return_observable_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    crate::register(&runtime)?;
    let pattern = make_string(&mut ctx, &runtime, &['/', 't', 'm', 'p', '/', '*'])?;
    let entries = call(&runtime, &mut ctx, "DIRECTORY", &[pattern])?;
    assert_ne!(entries, Word::NIL); // check-added-lines: allow(panic) directory result assertion

    let source = make_string(
        &mut ctx,
        &runtime,
        &['/', 't', 'm', 'p', '/', 'a', '.', 'l', 'i', 's', 'p'],
    )?;
    let from = make_string(
        &mut ctx,
        &runtime,
        &['/', 't', 'm', 'p', '/', '*', '.', 'l', 'i', 's', 'p'],
    )?;
    let to = make_string(
        &mut ctx,
        &runtime,
        &['/', 'v', 'a', 'r', '/', '*', '.', 'l', 'i', 's', 'p'],
    )?;
    let translated = call(
        &runtime,
        &mut ctx,
        "TRANSLATE-PATHNAME",
        &[source, from, to],
    )?;
    let translated_name = call(&runtime, &mut ctx, "NAMESTRING", &[translated])?;
    assert_eq!(string_value(&ctx, translated_name)?, "/var/a.lisp"); // check-added-lines: allow(panic) translation value assertion

    let home = call(&runtime, &mut ctx, "USER-HOMEDIR-PATHNAME", &[])?;
    assert_eq!(call(&runtime, &mut ctx, "PATHNAMEP", &[home])?, Word::TRUE); // check-added-lines: allow(panic) pathname predicate assertion
    Ok(())
}

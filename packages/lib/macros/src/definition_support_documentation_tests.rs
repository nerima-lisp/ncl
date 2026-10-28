use super::super::documentation::{documentation_builtin, set_documentation_builtin};
use super::*;
use crate::form::list;
use ncl_object::{BuiltinArgs, MultipleValues};

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

fn string(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Result<Word, ObjectError> {
    ncl_object::make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
}

fn call_documentation(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    documentation_builtin(
        ctx,
        runtime,
        &BuiltinArgs::new(arguments),
        &mut MultipleValues::new(),
    )
}

fn set_documentation(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    arguments: &[Word],
) -> Result<Word, ObjectError> {
    set_documentation_builtin(
        ctx,
        runtime,
        &BuiltinArgs::new(arguments),
        &mut MultipleValues::new(),
    )
}

#[test]
fn symbol_and_package_documentation_round_trip_with_type_checks() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "SETF-DOC-SYMBOL")?;
    let kind = symbol(&mut ctx, &runtime, "FUNCTION")?;
    let doc = string(&mut ctx, &runtime, "function docs")?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, kind])?,
        Word::NIL
    );
    assert_eq!(
        set_documentation(&mut ctx, &runtime, &[name, kind, doc])?,
        doc
    );
    assert_eq!(call_documentation(&mut ctx, &runtime, &[name, kind])?, doc);

    let updated = string(&mut ctx, &runtime, "updated docs")?;
    set_documentation(&mut ctx, &runtime, &[name, kind, updated])?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, kind])?,
        updated
    );

    let documentation_key = symbol(&mut ctx, &runtime, "NCL::DOCUMENTATION")?;
    let malformed_table = ncl_object::make_cons(&mut ctx, &runtime, kind, Word::fixnum(7))?;
    super::super::set_property(&mut ctx, &runtime, name, documentation_key, malformed_table)?;
    assert!(matches!(
        set_documentation(&mut ctx, &runtime, &[name, kind, doc]),
        Err(ObjectError::TypeError)
    ));
    super::super::set_property(&mut ctx, &runtime, name, documentation_key, Word::fixnum(7))?;
    assert!(matches!(
        set_documentation(&mut ctx, &runtime, &[name, kind, doc]),
        Err(ObjectError::TypeError)
    ));

    let package_word = runtime.ensure_package(&mut ctx, "SETF-DOC-PACKAGE")?;
    let package_doc = string(&mut ctx, &runtime, "package docs")?;
    set_documentation(&mut ctx, &runtime, &[package_word, Word::TRUE, package_doc])?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[package_word, Word::TRUE])?,
        package_doc
    );
    assert!(matches!(
        call_documentation(&mut ctx, &runtime, &[package_word, kind]),
        Err(ObjectError::TypeError)
    ));
    assert!(matches!(
        set_documentation(&mut ctx, &runtime, &[package_word, kind, package_doc]),
        Err(ObjectError::TypeError)
    ));
    assert!(matches!(
        set_documentation(&mut ctx, &runtime, &[name, kind, Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    ));
    assert!(matches!(
        call_documentation(&mut ctx, &runtime, &[Word::fixnum(1), kind]),
        Err(ObjectError::TypeError)
    ));
    assert!(matches!(
        set_documentation(&mut ctx, &runtime, &[Word::fixnum(1), kind, doc]),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}

#[test]
fn setf_documentation_reads_short_and_long_definition_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "SETF-DOC-FUNCTION")?;
    let definition_key = symbol(&mut ctx, &runtime, "NCL::DEFINITION")?;
    let setf_kind = symbol(&mut ctx, &runtime, "SETF")?;
    let setf_name = symbol(&mut ctx, &runtime, "SETF-DOC-FUNCTION")?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
        Word::NIL
    );
    let no_doc_definition = list(&mut ctx, &runtime, &[setf_name])?;
    super::super::set_property(&mut ctx, &runtime, name, definition_key, no_doc_definition)?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
        Word::NIL
    );
    let short_doc = string(&mut ctx, &runtime, "short definition docs")?;
    let short_definition = list(&mut ctx, &runtime, &[setf_name, short_doc])?;
    super::super::set_property(&mut ctx, &runtime, name, definition_key, short_definition)?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
        short_doc
    );

    let non_string_doc = list(&mut ctx, &runtime, &[setf_name, Word::fixnum(17)])?;
    super::super::set_property(&mut ctx, &runtime, name, definition_key, non_string_doc)?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
        Word::NIL
    );

    let variable = symbol(&mut ctx, &runtime, "X")?;
    let declare = symbol(&mut ctx, &runtime, "DECLARE")?;
    let special = symbol(&mut ctx, &runtime, "SPECIAL")?;
    let declaration_body = list(&mut ctx, &runtime, &[special, variable])?;
    let declaration = list(&mut ctx, &runtime, &[declare, declaration_body])?;
    let long_doc = string(&mut ctx, &runtime, "long definition docs")?;
    let long_definition = list(
        &mut ctx,
        &runtime,
        &[Word::NIL, Word::NIL, declaration, long_doc],
    )?;
    super::super::set_property(&mut ctx, &runtime, name, definition_key, long_definition)?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
        long_doc
    );

    let long_definition_without_doc = list(
        &mut ctx,
        &runtime,
        &[Word::NIL, Word::NIL, declaration, declaration],
    )?;
    super::super::set_property(
        &mut ctx,
        &runtime,
        name,
        definition_key,
        long_definition_without_doc,
    )?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
        Word::NIL
    );

    let non_declaration = symbol(&mut ctx, &runtime, "NOT-A-DECLARATION")?;
    let ignored_doc = string(&mut ctx, &runtime, "not leading")?;
    let trailing_definition = list(
        &mut ctx,
        &runtime,
        &[Word::NIL, Word::NIL, non_declaration, ignored_doc],
    )?;
    super::super::set_property(
        &mut ctx,
        &runtime,
        name,
        definition_key,
        trailing_definition,
    )?;
    assert_eq!(
        call_documentation(&mut ctx, &runtime, &[name, setf_kind])?,
        Word::NIL
    );
    Ok(())
}

use super::*;

#[test]
fn package_registry_resolution_reports_malformed_metadata() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut context = ThreadContext::new();
    context.register(&runtime)?;
    runtime.ensure_package(&mut context, "REGISTRY-METADATA")?;
    let malformed_key = make_string(&mut context, &runtime, &['M', 'A', 'L', 'F'])?;
    HashTable::from_word(Runtime::table(&runtime.packages)?).insert(
        &mut context,
        &runtime,
        malformed_key,
        Word::NIL,
    )?;
    assert_eq!(
        runtime.resolve_package(&context, "NO-SUCH-PACKAGE"),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.resolve_package_name(&context, "NO-SUCH-PACKAGE"),
        Err(ObjectError::TypeError)
    );

    let runtime = Runtime::new()?;
    let mut context = ThreadContext::new();
    context.register(&runtime)?;
    let package = runtime.ensure_package(&mut context, "REGISTRY-NICKNAMES")?;
    put(
        &mut context,
        package,
        crate::package::NICKNAMES,
        Word::fixnum(1),
    )?;
    assert_eq!(
        runtime.resolve_package(&context, "NO-SUCH-PACKAGE"),
        Err(ObjectError::Layout)
    );
    Ok(())
}

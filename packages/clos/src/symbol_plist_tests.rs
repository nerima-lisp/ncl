use super::*;

fn make_list<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    values: &[Word],
) -> Result<ncl_object::Handle<'ctx, Word>, ObjectError> {
    let roots = scope.root_many(
        &values
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    scope.make_list(runtime, &roots)
}

fn setup() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut context = ThreadContext::new();
    context.register(&runtime)?;
    Ok((runtime, context))
}

#[test]
fn symbol_plist_helpers_skip_other_indicators_and_preserve_values() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let name = scope.intern(&runtime, "COMMON-LISP-USER", "R6-B2-PLIST-HELPER-TEST")?;
    let first_key = scope.intern(&runtime, "COMMON-LISP-USER", "R6-B2-FIRST-PROPERTY")?;
    let target_key = scope.intern(&runtime, "COMMON-LISP-USER", "R6-B2-TARGET-PROPERTY")?;
    let added_key = scope.intern(&runtime, "COMMON-LISP-USER", "R6-B2-ADDED-PROPERTY")?;
    let missing_key = scope.intern(&runtime, "COMMON-LISP-USER", "R6-B2-MISSING-PROPERTY")?;
    let first_key_word = scope.get(first_key).as_word();
    let target_key_word = scope.get(target_key).as_word();
    let plist = make_list(
        &mut scope,
        &runtime,
        &[
            first_key_word,
            Word::fixnum(13),
            target_key_word,
            Word::fixnum(29),
        ],
    )?;
    let name_word = scope.get(name).as_word();
    let plist_word = scope.get(plist).as_word();
    ncl_object::set_symbol_plist(scope.context_mut(), name_word, plist_word)?;

    let target_value =
        symbol_plist_property(&mut scope, name, target_key)?.ok_or(ObjectError::TypeError)?;
    assert_eq!(scope.get(target_value).as_word(), Word::fixnum(29));

    let replacement = scope.root(Local::from_word(Word::fixnum(31)));
    set_symbol_plist_property(&mut scope, &runtime, name, target_key, replacement)?;
    let target_value =
        symbol_plist_property(&mut scope, name, target_key)?.ok_or(ObjectError::TypeError)?;
    assert_eq!(scope.get(target_value).as_word(), Word::fixnum(31));
    let first_value =
        symbol_plist_property(&mut scope, name, first_key)?.ok_or(ObjectError::TypeError)?;
    assert_eq!(scope.get(first_value).as_word(), Word::fixnum(13));

    let added_value = scope.root(Local::from_word(Word::fixnum(41)));
    set_symbol_plist_property(&mut scope, &runtime, name, added_key, added_value)?;
    let added =
        symbol_plist_property(&mut scope, name, added_key)?.ok_or(ObjectError::TypeError)?;
    assert_eq!(scope.get(added).as_word(), Word::fixnum(41));
    assert!(symbol_plist_property(&mut scope, name, missing_key)?.is_none());
    Ok(())
}

#[test]
fn symbol_plist_helpers_reject_an_incomplete_indicator_value_pair() -> Result<(), ObjectError> {
    let (runtime, mut context) = setup()?;
    let mut scope = Scope::new(&mut context);
    let name = scope.intern(&runtime, "COMMON-LISP-USER", "R6-B2-MALFORMED-PLIST-TEST")?;
    let key = scope.intern(&runtime, "COMMON-LISP-USER", "R6-B2-MALFORMED-KEY")?;
    let indicator = scope.intern(&runtime, "COMMON-LISP-USER", "R6-B2-ORPHAN-INDICATOR")?;
    let indicator_word = scope.get(indicator).as_word();
    let plist = make_list(&mut scope, &runtime, &[indicator_word])?;
    let name_word = scope.get(name).as_word();
    let plist_word = scope.get(plist).as_word();
    ncl_object::set_symbol_plist(scope.context_mut(), name_word, plist_word)?;

    assert!(matches!(
        symbol_plist_property(&mut scope, name, key),
        Err(ObjectError::TypeError)
    ));
    let value = scope.root(Local::from_word(Word::fixnum(5)));
    assert!(matches!(
        set_symbol_plist_property(&mut scope, &runtime, name, key, value),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}

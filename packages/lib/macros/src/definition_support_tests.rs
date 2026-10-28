use super::*;

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register_runtime_support(&mut ctx, &runtime)?;
    Ok((runtime, ctx))
}

#[test]
fn get_set_updates_existing_properties_and_appends_missing_properties() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "SETF-GET-TEST")?;
    let property = symbol(&mut ctx, &runtime, "SETF-GET-PROPERTY")?;

    for value in [Word::fixnum(1), Word::fixnum(2)] {
        let words = [name, property, value];
        let args = BuiltinArgs::new(&words);
        assert_eq!(
            set_get_builtin(
                &mut ctx,
                &runtime,
                &args,
                &mut ncl_object::MultipleValues::new(),
            )?,
            value
        );
        let words = [name, property];
        let args = BuiltinArgs::new(&words);
        assert_eq!(
            get_builtin(
                &mut ctx,
                &runtime,
                &args,
                &mut ncl_object::MultipleValues::new(),
            )?,
            value
        );
    }

    let second_property = symbol(&mut ctx, &runtime, "SETF-GET-SECOND-PROPERTY")?;
    let second_value = Word::fixnum(3);
    let words = [name, second_property, second_value];
    let args = BuiltinArgs::new(&words);
    assert_eq!(
        set_get_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        )?,
        second_value
    );
    let updated_second_value = Word::fixnum(4);
    let words = [name, second_property, updated_second_value];
    let args = BuiltinArgs::new(&words);
    assert_eq!(
        set_get_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        )?,
        updated_second_value
    );
    assert_eq!(get_property(&ctx, name, property)?, Word::fixnum(2));
    assert_eq!(
        get_property(&ctx, name, second_property)?,
        updated_second_value
    );
    let absent = symbol(&mut ctx, &runtime, "SETF-GET-ABSENT")?;
    let words = [name, absent];
    let args = BuiltinArgs::new(&words);
    assert_eq!(
        get_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        )?,
        Word::NIL
    );

    let malformed = symbol(&mut ctx, &runtime, "SETF-GET-MALFORMED")?;
    ncl_object::set_symbol_plist(&mut ctx, malformed, Word::fixnum(7))?;
    let words = [malformed, property, Word::fixnum(5)];
    let args = BuiltinArgs::new(&words);
    assert!(matches!(
        set_get_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        ),
        Err(ObjectError::TypeError)
    ));

    let malformed_tail = make_cons(&mut ctx, &runtime, property, Word::fixnum(7))?;
    ncl_object::set_symbol_plist(&mut ctx, malformed, malformed_tail)?;
    let words = [malformed, property, Word::fixnum(5)];
    let args = BuiltinArgs::new(&words);
    assert!(matches!(
        set_get_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        ),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}

#[test]
fn get_property_rejects_non_list_and_dotted_symbol_plists() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "SETF-GET-MALFORMED-READ")?;
    let property = symbol(&mut ctx, &runtime, "SETF-GET-MALFORMED-READ-PROPERTY")?;

    ncl_object::set_symbol_plist(&mut ctx, name, Word::fixnum(7))?;
    assert_eq!(
        get_property(&ctx, name, property),
        Err(ObjectError::TypeError)
    );

    let dotted_plist = make_cons(&mut ctx, &runtime, property, Word::fixnum(7))?;
    ncl_object::set_symbol_plist(&mut ctx, name, dotted_plist)?;
    assert_eq!(
        get_property(&ctx, name, property),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn function_definition_builtins_are_registered() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    assert!(
        runtime
            .function(&mut ctx, "COMMON-LISP", "FDEFINITION")
            .is_some()
    );
    assert!(
        runtime
            .function(&mut ctx, "NCL-EXT", "DOCUMENTATION-SET")
            .is_some()
    );
    Ok(())
}

#[test]
fn function_definition_builtin_sets_and_gets_function_objects() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "SETF-FDEFINITION-BUILTIN-TEST")?;
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "GET")
        .ok_or(ObjectError::Layout)?;

    let words = [name, function];
    let args = BuiltinArgs::new(&words);
    assert_eq!(
        set_fdefinition_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        )?,
        function
    );
    let words = [name];
    let args = BuiltinArgs::new(&words);
    assert_eq!(
        fdefinition_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        )?,
        function
    );
    let unbound = symbol(&mut ctx, &runtime, "SETF-FDEFINITION-UNBOUND")?;
    let words = [unbound];
    let args = BuiltinArgs::new(&words);
    assert!(matches!(
        fdefinition_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        ),
        Err(ObjectError::Unbound)
    ));
    Ok(())
}

#[test]
fn macro_function_builtin_sets_and_gets_macro_function_objects() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let macro_name = symbol(&mut ctx, &runtime, "SETF-MACRO-FUNCTION-BUILTIN-TEST")?;
    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "GET")
        .ok_or(ObjectError::Layout)?;

    let words = [macro_name];
    let args = BuiltinArgs::new(&words);
    assert_eq!(
        macro_function_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        )?,
        Word::NIL
    );

    let words = [macro_name, function];
    let args = BuiltinArgs::new(&words);
    assert_eq!(
        set_macro_function_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        )?,
        function
    );
    let words = [macro_name];
    let args = BuiltinArgs::new(&words);
    assert_eq!(
        macro_function_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        )?,
        function
    );
    Ok(())
}

#[test]
fn function_definition_builtins_reject_invalid_names_and_values() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "SETF-FDEFINITION-BUILTIN-TEST")?;

    let words = [name, Word::fixnum(1)];
    let args = BuiltinArgs::new(&words);
    assert!(matches!(
        set_fdefinition_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        ),
        Err(ObjectError::TypeError)
    ));
    let words = [Word::fixnum(1)];
    let args = BuiltinArgs::new(&words);
    assert!(matches!(
        fdefinition_builtin(
            &mut ctx,
            &runtime,
            &args,
            &mut ncl_object::MultipleValues::new(),
        ),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}

#[test]
fn definition_and_get_places_bind_their_arguments_once() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = symbol(&mut ctx, &runtime, "SETF-DEFINITION-PLACE")?;
    let property = symbol(&mut ctx, &runtime, "SETF-GET-PLACE")?;

    let definition = fdefinition_place(&mut ctx, &runtime, &[name])?;
    assert_eq!(definition.temporary_variables.len(), 1);
    assert_eq!(definition.value_forms, vec![name]);
    assert_eq!(definition.store_variables.len(), 1);
    let definition_temporary = definition.temporary_variables[0];
    let definition_accessor = symbol(&mut ctx, &runtime, "FDEFINITION")?;
    let definition_setter = symbol(&mut ctx, &runtime, "NCL-EXT::FDEFINITION-SET")?;
    assert_eq!(
        crate::elements(&mut ctx, definition.access_form)?,
        vec![definition_accessor, definition_temporary]
    );
    assert_eq!(
        crate::elements(&mut ctx, definition.store_form)?,
        vec![
            definition_setter,
            definition_temporary,
            definition.store_variables[0]
        ]
    );
    assert!(matches!(
        fdefinition_place(&mut ctx, &runtime, &[]),
        Err(ObjectError::TypeError)
    ));

    let environment = Word::NIL;
    let macro_function = macro_function_place(&mut ctx, &runtime, &[name, environment])?;
    assert_eq!(macro_function.temporary_variables.len(), 2);
    assert_eq!(macro_function.value_forms, vec![name, environment]);
    assert_eq!(macro_function.store_variables.len(), 1);
    let macro_temporary = macro_function.temporary_variables[0];
    let macro_accessor = symbol(&mut ctx, &runtime, "MACRO-FUNCTION")?;
    let macro_setter = symbol(&mut ctx, &runtime, "NCL-EXT::MACRO-FUNCTION-SET")?;
    assert_eq!(
        crate::elements(&mut ctx, macro_function.access_form)?,
        vec![macro_accessor, macro_temporary]
    );
    assert_eq!(
        crate::elements(&mut ctx, macro_function.store_form)?,
        vec![
            macro_setter,
            macro_temporary,
            macro_function.store_variables[0]
        ]
    );
    assert!(matches!(
        macro_function_place(&mut ctx, &runtime, &[name, property, name]),
        Err(ObjectError::TypeError)
    ));

    let get = get_place(&mut ctx, &runtime, &[name, property])?;
    assert_eq!(get.temporary_variables.len(), 2);
    assert_eq!(get.value_forms, vec![name, property]);
    assert_eq!(get.store_variables.len(), 1);
    assert!(matches!(
        get_place(&mut ctx, &runtime, &[name]),
        Err(ObjectError::TypeError)
    ));
    Ok(())
}

mod documentation_tests {
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

    fn string(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        value: &str,
    ) -> Result<Word, ObjectError> {
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
}

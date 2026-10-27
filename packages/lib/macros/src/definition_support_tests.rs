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
fn function_definition_builtins_set_get_and_validate_function_objects() -> Result<(), ObjectError> {
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
    let macro_name = symbol(&mut ctx, &runtime, "SETF-MACRO-FUNCTION-BUILTIN-TEST")?;
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

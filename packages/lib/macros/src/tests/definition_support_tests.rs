use super::*;
use crate::elements;

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register_runtime_support(&mut ctx, &runtime)?;
    Ok((runtime, ctx))
}

#[test]
fn function_definition_builtins_return_functions_and_reject_bad_designators(
) -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let car = crate::symbol(&mut ctx, &runtime, "FDEFINITION")?;
    let not_symbol = Word::fixnum(1);
    let mut values = ncl_object::MultipleValues::new();
    let car_values = [car];
    let car_args = BuiltinArgs::new(&car_values);
    let function = fdefinition_builtin(&mut ctx, &runtime, &car_args, &mut values)?;
    assert!(FunctionObject::try_from(function).is_ok());
    assert_eq!(
        fdefinition_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[not_symbol]),
            &mut values
        ),
        Err(ObjectError::TypeError)
    );
    let unbound = crate::symbol(&mut ctx, &runtime, "UNBOUND-FUNCTION")?;
    assert_eq!(
        fdefinition_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[unbound]),
            &mut values
        ),
        Err(ObjectError::Unbound)
    );
    assert_eq!(
        macro_function_builtin(&mut ctx, &runtime, &car_args, &mut values)?,
        function
    );
    assert_eq!(
        macro_function_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[unbound]),
            &mut values
        )?,
        Word::NIL
    );
    Ok(())
}

#[test]
fn set_definition_and_macro_function_update_symbol_cells() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = crate::symbol(&mut ctx, &runtime, "LOCAL-FUNCTION")?;
    let car = crate::symbol(&mut ctx, &runtime, "FDEFINITION")?;
    let function = symbol_function(&ctx, car)?;
    let mut values = ncl_object::MultipleValues::new();
    let set_values = [name, function];
    let set_args = BuiltinArgs::new(&set_values);
    assert_eq!(
        set_fdefinition_builtin(&mut ctx, &runtime, &set_args, &mut values)?,
        function
    );
    assert_eq!(
        fdefinition_builtin(&mut ctx, &runtime, &BuiltinArgs::new(&[name]), &mut values)?,
        function
    );
    let macro_name = crate::symbol(&mut ctx, &runtime, "LOCAL-MACRO")?;
    assert_eq!(
        set_macro_function_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[macro_name, function]),
            &mut values
        )?,
        function
    );
    assert_eq!(
        macro_function_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[macro_name]),
            &mut values
        )?,
        function
    );
    assert!(ncl_object::symbol_is_macro(&ctx, macro_name)?);
    assert_eq!(
        set_fdefinition_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[name, Word::NIL]),
            &mut values
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn get_and_set_get_builtins_handle_missing_existing_and_malformed_properties(
) -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = crate::symbol(&mut ctx, &runtime, "PROPERTY-HOLDER")?;
    let key = crate::symbol(&mut ctx, &runtime, "KEY")?;
    let value = Word::fixnum(42);
    let mut values = ncl_object::MultipleValues::new();
    assert_eq!(
        get_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[name, key]),
            &mut values
        )?,
        Word::NIL
    );
    assert_eq!(
        set_get_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[name, key, value]),
            &mut values
        )?,
        value
    );
    assert_eq!(
        get_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[name, key]),
            &mut values
        )?,
        value
    );
    let replacement = Word::fixnum(99);
    assert_eq!(
        set_get_builtin(
            &mut ctx,
            &runtime,
            &BuiltinArgs::new(&[name, key, replacement]),
            &mut values
        )?,
        replacement
    );
    assert_eq!(get_property(&ctx, name, key)?, replacement);
    let malformed_name = crate::symbol(&mut ctx, &runtime, "MALFORMED")?;
    ncl_object::set_symbol_plist(&mut ctx, malformed_name, Word::TRUE)?;
    assert_eq!(
        get_property(&ctx, malformed_name, key),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn place_expanders_produce_access_and_store_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let name = crate::symbol(&mut ctx, &runtime, "F")?;
    let property = crate::symbol(&mut ctx, &runtime, "P")?;
    let fdefinition = fdefinition_place(&mut ctx, &runtime, &[name])?;
    assert_eq!(
        elements(&mut ctx, fdefinition.access_form)?[0],
        crate::symbol(&mut ctx, &runtime, "FDEFINITION")?
    );
    assert_eq!(fdefinition.store_variables.len(), 1);
    let macro_function = macro_function_place(&mut ctx, &runtime, &[name])?;
    assert_eq!(
        elements(&mut ctx, macro_function.access_form)?[0],
        crate::symbol(&mut ctx, &runtime, "MACRO-FUNCTION")?
    );
    let get = get_place(&mut ctx, &runtime, &[name, property])?;
    assert_eq!(get.temporary_variables.len(), 2);
    assert_eq!(
        elements(&mut ctx, get.access_form)?[0],
        crate::symbol(&mut ctx, &runtime, "GET")?
    );
    Ok(())
}

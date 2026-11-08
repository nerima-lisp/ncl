use super::{
    BuiltinFunctionCaller, Local, METHOD_COMBINATION_DEFINITION_KEY_NAME,
    METHOD_COMBINATION_FUNCTION_KEY_NAME, MultipleValues, ObjectError, Runtime, Scope,
    dispatch_list_to_handles, lib_dispatch_effective_methods, lib_dispatch_selection,
    method_registry, symbol_function, symbol_plist_property,
};

pub struct CustomMethodCombinationDispatch<'args, 'ctx> {
    pub(super) generic_name: ncl_object::Handle<'ctx>,
    pub(super) combination_name: ncl_object::Handle<'ctx>,
    pub(super) arguments: &'args ncl_object::HandleVec<'ctx>,
    pub(super) argument_list: ncl_object::Handle<'ctx>,
}

pub fn dispatch_custom_method_combination<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    dispatch: &CustomMethodCombinationDispatch<'_, 'ctx>,
    values: &mut MultipleValues,
) -> Result<Option<ncl_object::Handle<'ctx>>, ObjectError> {
    let function_key = scope.intern(runtime, "NCL", METHOD_COMBINATION_FUNCTION_KEY_NAME)?;
    let Some(helper_name) = symbol_plist_property(scope, dispatch.combination_name, function_key)?
    else {
        return Ok(None);
    };
    let definition_key = scope.intern(runtime, "NCL", METHOD_COMBINATION_DEFINITION_KEY_NAME)?;
    let definition = symbol_plist_property(scope, dispatch.combination_name, definition_key)?
        .ok_or(ObjectError::TypeError)?;
    let definition_parts = dispatch_list_to_handles(scope, definition)?;
    let group_specs = *definition_parts
        .as_slice()
        .get(3)
        .ok_or(ObjectError::TypeError)?;
    let registry = method_registry(scope, runtime, dispatch.generic_name)?;
    let entries = dispatch_list_to_handles(scope, registry)?;
    let mut caller = BuiltinFunctionCaller;
    let groups = lib_dispatch_selection::collect_custom_method_groups(
        scope,
        runtime,
        group_specs,
        dispatch.arguments,
        &entries,
        &mut caller,
        values,
    )?;
    let helper_word = symbol_function(scope.context(), scope.get(helper_name).as_word())?;
    let helper = scope.root(Local::from_word(helper_word));
    let result = lib_dispatch_effective_methods::invoke_custom_effective_method(
        scope,
        runtime,
        helper,
        &groups,
        dispatch.argument_list,
        &mut caller,
        values,
    )?;
    Ok(Some(result))
}

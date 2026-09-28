use super::{
    BuiltinIdentifier, BuiltinName, BuiltinPackage, ObjectRuntime, RuntimeError,
    keyword_check_native, keyword_supplied_native, keyword_value_native,
};

pub(super) fn install_keyword_builtins(object: &ObjectRuntime) -> Result<(), RuntimeError> {
    let addresses = [
        (
            "check-keywords",
            ncl_sys::function_address!(keyword_check_native),
        ),
        (
            "keyword-value",
            ncl_sys::function_address!(keyword_value_native),
        ),
        (
            "keyword-supplied-p",
            ncl_sys::function_address!(keyword_supplied_native),
        ),
    ];
    for (name, address) in addresses {
        object.register_builtin_address(
            BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new(name)),
            usize::try_from(address.map_err(|error| RuntimeError::Native(error.to_string()))?)
                .map_err(|_| RuntimeError::Native("builtin address overflow".to_owned()))?,
        );
    }
    Ok(())
}

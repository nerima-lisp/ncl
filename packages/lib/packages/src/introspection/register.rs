use super::{
    Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, NAME, ONE_FIND_PACKAGE_NAME, ONE_NAME, ONE_OBJECT, ONE_PACKAGE,
    ONE_SYMBOLS, ObjectError, Parameter, Runtime, ThreadContext, export, find_all_symbols,
    find_package, find_symbol, import, intern, list_all_packages, package_error_package,
    package_management, package_name, package_nicknames, package_shadowing_symbols,
    package_use_list, package_used_by_list, packagep, shadow, unexport, unintern, unuse_package,
    use_package,
};

const NO_PARAMETERS: &[Parameter] = &[];
type BuiltinRegistration = (
    &'static str,
    &'static [Parameter],
    &'static [Parameter],
    ncl_object::RustBuiltin,
);
const REGISTRATIONS: &[BuiltinRegistration] = &[
    (
        "FIND-PACKAGE",
        ONE_FIND_PACKAGE_NAME,
        NO_PARAMETERS,
        find_package,
    ),
    ("PACKAGE-NAME", ONE_PACKAGE, NO_PARAMETERS, package_name),
    (
        "PACKAGE-NICKNAMES",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_nicknames,
    ),
    (
        "PACKAGE-SHADOWING-SYMBOLS",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_shadowing_symbols,
    ),
    (
        "PACKAGE-USE-LIST",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_use_list,
    ),
    (
        "PACKAGE-USED-BY-LIST",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_used_by_list,
    ),
    ("PACKAGEP", ONE_OBJECT, NO_PARAMETERS, packagep),
    (
        "LIST-ALL-PACKAGES",
        NO_PARAMETERS,
        NO_PARAMETERS,
        list_all_packages,
    ),
    (
        "FIND-ALL-SYMBOLS",
        ONE_NAME,
        NO_PARAMETERS,
        find_all_symbols,
    ),
    (
        "PACKAGE-ERROR-PACKAGE",
        ONE_OBJECT,
        NO_PARAMETERS,
        package_error_package,
    ),
    ("FIND-SYMBOL", ONE_NAME, ONE_PACKAGE, find_symbol),
    ("INTERN", ONE_NAME, ONE_PACKAGE, intern),
    ("EXPORT", ONE_SYMBOLS, ONE_PACKAGE, export),
    ("UNEXPORT", ONE_SYMBOLS, ONE_PACKAGE, unexport),
    ("UNINTERN", ONE_NAME, ONE_PACKAGE, unintern),
    ("IMPORT", ONE_OBJECT, ONE_PACKAGE, import),
    ("SHADOW", ONE_SYMBOLS, ONE_PACKAGE, shadow),
    ("USE-PACKAGE", ONE_OBJECT, ONE_PACKAGE, use_package),
    ("UNUSE-PACKAGE", ONE_OBJECT, ONE_PACKAGE, unuse_package),
    (
        "DELETE-PACKAGE",
        ONE_PACKAGE,
        NO_PARAMETERS,
        package_management::delete_package,
    ),
    (
        "SHADOWING-IMPORT",
        ONE_SYMBOLS,
        ONE_PACKAGE,
        package_management::shadowing_import,
    ),
];

const fn descriptor(required: &'static [Parameter], optional: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::with_optional(required, optional),
        convention: BuiltinConvention::Adapted,
    }
}

/// Register package introspection and mutation operations backed by the object API.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for &(name, required, optional, function) in REGISTRATIONS {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor(required, optional), function),
        )?;
    }
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("MAKE-PACKAGE")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::with_rest(
                    &[NAME],
                    package_management::MAKE_PACKAGE_OPTIONS,
                ),
                convention: BuiltinConvention::Adapted,
            },
            package_management::make_package,
        ),
    )?;
    runtime.register_builtin(
        &mut ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("RENAME-PACKAGE"),
        ),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::with_optional(
                    package_management::RENAME_PACKAGE_PARAMETERS,
                    &[package_management::NEW_NICKNAMES],
                ),
                convention: BuiltinConvention::Adapted,
            },
            package_management::rename_package,
        ),
    )?;
    Ok(())
}

use super::{
    Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, NAME, ONE_OBJECT, ONE_PACKAGE, ObjectError, PACKAGE_PACKAGE,
    Parameter, Runtime, SYMBOLS_PACKAGE, ThreadContext, export, find_all_symbols, find_package,
    find_symbol, import, intern, list_all_packages, package_error_package, package_management,
    package_name, package_nicknames, package_shadowing_symbols, package_use_list,
    package_used_by_list, packagep, shadow, unexport, unintern, unuse_package, use_package,
};

const fn descriptor(required: &'static [Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: BuiltinConvention::Adapted,
    }
}

/// Register package introspection and mutation operations backed by the object API.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for (name, params, function) in [
        (
            "FIND-PACKAGE",
            &[NAME][..],
            find_package as ncl_object::RustBuiltin,
        ),
        ("PACKAGE-NAME", ONE_PACKAGE, package_name),
        ("PACKAGE-NICKNAMES", ONE_PACKAGE, package_nicknames),
        (
            "PACKAGE-SHADOWING-SYMBOLS",
            ONE_PACKAGE,
            package_shadowing_symbols,
        ),
        ("PACKAGE-USE-LIST", ONE_PACKAGE, package_use_list),
        ("PACKAGE-USED-BY-LIST", ONE_PACKAGE, package_used_by_list),
        ("PACKAGEP", ONE_OBJECT, packagep),
        ("LIST-ALL-PACKAGES", &[][..], list_all_packages),
        ("FIND-ALL-SYMBOLS", &[NAME][..], find_all_symbols),
        ("PACKAGE-ERROR-PACKAGE", ONE_OBJECT, package_error_package),
        ("USE-PACKAGE", PACKAGE_PACKAGE, use_package),
        ("UNUSE-PACKAGE", PACKAGE_PACKAGE, unuse_package),
        (
            "DELETE-PACKAGE",
            ONE_PACKAGE,
            package_management::delete_package,
        ),
        (
            "SHADOWING-IMPORT",
            SYMBOLS_PACKAGE,
            package_management::shadowing_import,
        ),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor(params), function),
        )?;
    }
    for (name, function) in [
        ("EXPORT", export as ncl_object::RustBuiltin),
        ("UNEXPORT", unexport),
        ("IMPORT", import),
        ("SHADOW", shadow),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::with_optional(&[super::SYMBOLS], &[super::PACKAGE]),
                    convention: BuiltinConvention::Adapted,
                },
                function,
            ),
        )?;
    }
    for (name, function) in [
        ("FIND-SYMBOL", find_symbol as ncl_object::RustBuiltin),
        ("INTERN", intern),
        ("UNINTERN", unintern),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::with_optional(&[super::NAME], &[super::PACKAGE]),
                    convention: BuiltinConvention::Adapted,
                },
                function,
            ),
        )?;
    }
    register_package_constructors(runtime, &mut ctx)?;
    Ok(())
}

fn register_package_constructors(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
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
        ctx,
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

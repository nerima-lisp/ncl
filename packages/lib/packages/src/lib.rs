//! Typed package-lock builtins.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, MultipleValues, ObjectError, Package, Parameter, ParameterType,
    Runtime, ThreadContext, Word,
};

const PACKAGE: Parameter = Parameter {
    name: BuiltinName::new("PACKAGE"),
    ty: ParameterType::PackageDesignator,
};

fn package_arg(ctx: &ThreadContext, args: BuiltinArgs<'_>) -> Result<Package, ObjectError> {
    let word = args.required(0)?;
    Package::try_from_word(ctx, word)
}

fn package_locked(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if package_arg(ctx, *args)?.is_locked(ctx)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn lock_package(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let package = package_arg(ctx, *args)?;
    package.set_locked(ctx, true)?;
    Ok(package.as_word())
}

fn unlock_package(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let package = package_arg(ctx, *args)?;
    package.set_locked(ctx, false)?;
    Ok(package.as_word())
}

const fn descriptor() -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(&[PACKAGE]),
        convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
    }
}

/// Register the NCL package-lock extension operations.
///
/// # Errors
/// Returns an object-layer error if builtin registration fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let descriptor = descriptor();
    for (name, function) in [
        (
            "PACKAGE-LOCKED-P",
            package_locked as ncl_object::RustBuiltin,
        ),
        ("LOCK-PACKAGE", lock_package as ncl_object::RustBuiltin),
        ("UNLOCK-PACKAGE", unlock_package as ncl_object::RustBuiltin),
    ] {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::NclExt, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor, function),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_object::FunctionObject;

    #[test]
    fn lock_operations_round_trip() {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("thread: {error:?}"));
        register(&runtime).unwrap_or_else(|error| panic!("register: {error:?}"));
        let package = runtime
            .ensure_package(&mut ctx, "N25-LOCK")
            .unwrap_or_else(|error| panic!("package: {error:?}"));
        let lock = runtime
            .function(&mut ctx, "NCL-EXT", "LOCK-PACKAGE")
            .unwrap_or_else(|| panic!("lock"));
        let locked = runtime
            .function(&mut ctx, "NCL-EXT", "PACKAGE-LOCKED-P")
            .unwrap_or_else(|| panic!("locked"));
        let unlock = runtime
            .function(&mut ctx, "NCL-EXT", "UNLOCK-PACKAGE")
            .unwrap_or_else(|| panic!("unlock"));
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                FunctionObject::try_from(lock).unwrap_or_else(|e| panic!("lock fn: {e:?}")),
                &[package]
            ),
            Ok(package)
        );
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                FunctionObject::try_from(locked).unwrap_or_else(|e| panic!("locked fn: {e:?}")),
                &[package]
            ),
            Ok(Word::TRUE)
        );
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                FunctionObject::try_from(unlock).unwrap_or_else(|e| panic!("unlock fn: {e:?}")),
                &[package]
            ),
            Ok(package)
        );
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                FunctionObject::try_from(locked).unwrap_or_else(|e| panic!("locked fn: {e:?}")),
                &[package]
            ),
            Ok(Word::NIL)
        );
    }
}

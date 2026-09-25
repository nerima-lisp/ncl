//! Builtin registration for packages and symbols.

use ncl_object::{
    Builtin, BuiltinImplementation, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
};

fn package_locked(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if ncl_object::Package::from(args[0]).is_locked(ctx)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn set_package_lock(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
    locked: bool,
) -> Result<Word, ObjectError> {
    ncl_object::Package::from(args[0]).set_locked(ctx, locked)?;
    Ok(args[0])
}

fn lock_package(
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    set_package_lock(ctx, args, values, true)
}

fn unlock_package(
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    set_package_lock(ctx, args, values, false)
}

fn unsupported(
    _: &mut ThreadContext,
    _: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Err(ObjectError::Unsupported)
}

const COMMON_LISP_FUNCTIONS: &[(&str, u8)] = &[
    ("BOUNDP", 1),
    ("COPY-SYMBOL", 1),
    ("DELETE-PACKAGE", 1),
    ("EXPORT", 2),
    ("FBOUNDP", 1),
    ("FIND-ALL-SYMBOLS", 1),
    ("FIND-PACKAGE", 1),
    ("FIND-SYMBOL", 2),
    ("FMAKUNBOUND", 1),
    ("GENSYM", 0),
    ("GENTEMP", 0),
    ("GET", 2),
    ("IMPORT", 2),
    ("INTERN", 2),
    ("LIST-ALL-PACKAGES", 0),
    ("MAKE-PACKAGE", 1),
    ("MAKE-SYMBOL", 1),
    ("MAKUNBOUND", 1),
    ("PACKAGE-ERROR-PACKAGE", 1),
    ("PACKAGE-NAME", 1),
    ("PACKAGE-NICKNAMES", 1),
    ("PACKAGE-SHADOWING-SYMBOLS", 1),
    ("PACKAGE-USE-LIST", 1),
    ("PACKAGE-USED-BY-LIST", 1),
    ("PACKAGEP", 1),
    ("REMPROP", 2),
    ("RENAME-PACKAGE", 2),
    ("SET", 2),
    ("SHADOW", 2),
    ("SHADOWING-IMPORT", 2),
    ("SYMBOL-FUNCTION", 1),
    ("SYMBOL-NAME", 1),
    ("SYMBOL-PACKAGE", 1),
    ("SYMBOL-PLIST", 1),
    ("SYMBOL-VALUE", 1),
    ("SYMBOLP", 1),
    ("UNEXPORT", 2),
    ("UNINTERN", 2),
    ("UNUSE-PACKAGE", 2),
    ("USE-PACKAGE", 2),
];

const NCL_EXT_FUNCTIONS: &[(&str, u8)] = &[
    ("ADD-PACKAGE-LOCAL-NICKNAME", 3),
    ("REMOVE-PACKAGE-LOCAL-NICKNAME", 2),
    ("PACKAGE-LOCAL-NICKNAMES", 1),
    ("LOCK-PACKAGE", 1),
    ("UNLOCK-PACKAGE", 1),
    ("PACKAGE-LOCKED-P", 1),
];

/// Register package and symbol operations owned by this crate.
///
/// The function objects are real builtin objects. Operations not yet exposed
/// by the object layer report `Unsupported`.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for (name, arity) in COMMON_LISP_FUNCTIONS {
        runtime.register_builtin(
            &mut ctx,
            "COMMON-LISP",
            name,
            BuiltinImplementation::direct(
                Builtin {
                    arity: *arity,
                    direct: true,
                    lambda_list: "package operation",
                },
                unsupported,
            ),
        )?;
    }
    for (name, arity) in NCL_EXT_FUNCTIONS {
        runtime.register_builtin(
            &mut ctx,
            "NCL-EXT",
            name,
            BuiltinImplementation::direct(
                Builtin {
                    arity: *arity,
                    direct: true,
                    lambda_list: "package operation",
                },
                match *name {
                    "PACKAGE-LOCKED-P" => package_locked,
                    "LOCK-PACKAGE" => lock_package,
                    "UNLOCK-PACKAGE" => unlock_package,
                    _ => unsupported,
                },
            ),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_object::FunctionObject;

    #[test]
    fn package_lock_builtins_change_and_read_lock_state() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("thread");
        register(&runtime).expect("register");
        let package = runtime
            .ensure_package(&mut ctx, "LOCK-TEST")
            .expect("package");
        let lock = FunctionObject::from(
            runtime
                .function(&mut ctx, "NCL-EXT", "LOCK-PACKAGE")
                .expect("lock"),
        );
        let unlock = FunctionObject::from(
            runtime
                .function(&mut ctx, "NCL-EXT", "UNLOCK-PACKAGE")
                .expect("unlock"),
        );
        let locked = FunctionObject::from(
            runtime
                .function(&mut ctx, "NCL-EXT", "PACKAGE-LOCKED-P")
                .expect("locked"),
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, lock, &[package]),
            Ok(package)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, locked, &[package]),
            Ok(Word::TRUE)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, unlock, &[package]),
            Ok(package)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, locked, &[package]),
            Ok(Word::NIL)
        );
        let unsupported = FunctionObject::from(
            runtime
                .function(&mut ctx, "COMMON-LISP", "BOUNDP")
                .expect("boundp"),
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, unsupported, &[Word::NIL]),
            Err(ObjectError::Unsupported)
        );
    }
}

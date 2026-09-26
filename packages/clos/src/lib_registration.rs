fn bind(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: BuiltinPackage,
    name: &'static str,
    arity: u8,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(package, BuiltinName::new(name)),
        BuiltinImplementation::direct(descriptor(arity), function),
    )?;
    Ok(())
}

fn register_classes(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    for name in [
        "CLASS",
        "NULL",
        "STANDARD-OBJECT",
        "STANDARD-CLASS",
        "BUILT-IN-CLASS",
        "STRUCTURE-CLASS",
        "GENERIC-FUNCTION",
        "STANDARD-GENERIC-FUNCTION",
        "METHOD",
        "STANDARD-METHOD",
        "METHOD-COMBINATION",
        "STRUCTURE-OBJECT",
        "EQL-SPECIALIZER",
        "T",
        "NUMBER",
        "INTEGER",
        "LIST",
        "CONS",
        "SYMBOL",
        "STRING",
        "VECTOR",
        "ARRAY",
        "HASH-TABLE",
        "STREAM",
        "PACKAGE",
        "FUNCTION",
        "CHARACTER",
        "SIMPLE-VECTOR",
        "BIGNUM",
        "RATIO",
        "DOUBLE-FLOAT",
        "COMPLEX",
    ] {
        let package = runtime.ensure_package(ctx, COMMON_LISP)?;
        Package::from_word(package).intern(ctx, runtime, name)?;
    }
    install_class(ctx, runtime, "T", None)?;
    install_class(ctx, runtime, "NULL", Some("T"))?;
    for &(name, superclass) in &[
        ("CLASS", Some("T")),
        ("STANDARD-OBJECT", Some("T")),
        ("STANDARD-CLASS", Some("CLASS")),
        ("BUILT-IN-CLASS", Some("CLASS")),
        ("STRUCTURE-CLASS", Some("CLASS")),
        ("GENERIC-FUNCTION", Some("FUNCTION")),
        ("STANDARD-GENERIC-FUNCTION", Some("GENERIC-FUNCTION")),
        ("METHOD", Some("STANDARD-OBJECT")),
        ("STANDARD-METHOD", Some("METHOD")),
        ("METHOD-COMBINATION", Some("STANDARD-OBJECT")),
        ("STRUCTURE-OBJECT", Some("STANDARD-OBJECT")),
        ("EQL-SPECIALIZER", Some("STANDARD-OBJECT")),
    ] {
        if runtime.class(ctx, name).is_none() {
            install_class(ctx, runtime, name, superclass)?;
        }
    }
    for name in [
        "NUMBER",
        "INTEGER",
        "LIST",
        "CONS",
        "SYMBOL",
        "STRING",
        "VECTOR",
        "ARRAY",
        "HASH-TABLE",
        "STREAM",
        "PACKAGE",
        "FUNCTION",
        "CHARACTER",
        "SIMPLE-VECTOR",
        "BIGNUM",
        "RATIO",
        "DOUBLE-FLOAT",
        "COMPLEX",
    ] {
        if runtime.class(ctx, name).is_none() {
            install_class(ctx, runtime, name, Some("T"))?;
        }
    }
    Ok(())
}

fn register_owned_symbols(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    for row in include_str!("../ownership.tsv").lines().skip(1) {
        let fields: Vec<_> = row.split('\t').collect();
        if fields.len() != 7 {
            return Err(ObjectError::TypeError);
        }
        match fields[0] {
            COMMON_LISP | NCL_MOP => {}
            _ => return Err(ObjectError::TypeError),
        }

        match fields[2] {
            "class" | "special-operator+class" | "constant+class" => {
                let package = runtime.ensure_package(ctx, fields[0])?;
                Package::from_word(package).intern(ctx, runtime, fields[1])?;
                if runtime.class(ctx, fields[1]).is_none() {
                    install_class(ctx, runtime, fields[1], Some("T"))?;
                }
            }
            "other" => {}
            "function" => {
                let package = runtime.ensure_package(ctx, fields[0])?;
                Package::from_word(package).intern(ctx, runtime, fields[1])?;
                let binding: Option<(u8, ncl_object::RustBuiltin)> = match (fields[0], fields[1]) {
                    (COMMON_LISP, "SLOT-BOUNDP") => Some((2, slot_boundp_builtin)),
                    (COMMON_LISP, "SLOT-EXISTS-P") => Some((2, slot_exists_builtin)),
                    (COMMON_LISP, "SLOT-MAKUNBOUND") => Some((2, slot_makunbound_builtin)),
                    (COMMON_LISP, "SLOT-VALUE") => Some((2, slot_value_builtin)),
                    (COMMON_LISP, "CLASS-OF") => Some((1, class_of_builtin)),
                    (COMMON_LISP, "CLASS-NAME") | (NCL_MOP, "CLASS-NAME") => {
                        Some((1, class_name_builtin))
                    }
                    _ if is_registered_elsewhere(fields[0], fields[1]) => None,
                    _ => return Err(ObjectError::TypeError),
                };
                if let Some((arity, callback)) = binding {
                    let builtin_package = match fields[0] {
                        COMMON_LISP => BuiltinPackage::CommonLisp,
                        NCL_MOP => BuiltinPackage::NclMop,
                        _ => return Err(ObjectError::TypeError),
                    };
                    bind(ctx, runtime, builtin_package, fields[1], arity, callback)?;
                }
            }
            _ => return Err(ObjectError::TypeError),
        }
    }
    bind(
        ctx,
        runtime,
        BuiltinPackage::CommonLisp,
        "SLOT-VALUE-SET",
        3,
        slot_set_builtin,
    )
}

fn is_registered_elsewhere(package: &str, name: &str) -> bool {
    matches!(
        (package, name),
        (COMMON_LISP, "MAKE-INSTANCE")
            | (COMMON_LISP, "INITIALIZE-INSTANCE")
            | (COMMON_LISP, "SHARED-INITIALIZE")
            | (NCL_MOP, "MAKE-INSTANCE")
            | (NCL_MOP, "CLASS-DIRECT-SLOTS")
            | (NCL_MOP, "CLASS-PRECEDENCE-LIST")
            | (NCL_MOP, "CLASS-SLOTS")
            | (NCL_MOP, "SLOT-DEFINITION-NAME")
            | (NCL_MOP, "SLOT-DEFINITION-LOCATION")
            | (NCL_MOP, "SLOT-VALUE-USING-CLASS")
            | (NCL_MOP, "SLOT-BOUNDP-USING-CLASS")
            | (NCL_MOP, "SLOT-MAKUNBOUND-USING-CLASS")
            | (NCL_MOP, "EQL-SPECIALIZER-OBJECT")
    )
}

/// Register CLOS classes, NCL-MOP names, and the implemented slot builtins.
///
/// # Errors
///
/// Returns the first object allocation, package, or builtin registration error.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    runtime.ensure_package(&mut ctx, NCL_MOP)?;
    register_classes(&mut ctx, runtime)?;
    for descriptor in mop::builtin_descriptors() {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(descriptor.package, descriptor.name),
            mop::implementation(*descriptor),
        )?;
    }
    initialization::register_initialization_builtins(runtime)?;
    register_owned_symbols(&mut ctx, runtime)?;
    Ok(())
}

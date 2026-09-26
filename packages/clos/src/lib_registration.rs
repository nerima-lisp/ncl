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
        if fields.len() < 3 {
            continue;
        }
        let package = runtime.ensure_package(ctx, fields[0])?;
        Package::from_word(package).intern(ctx, runtime, fields[1])?;
        if fields[2] == "class" {
            if runtime.class(ctx, fields[1]).is_none() {
                install_class(ctx, runtime, fields[1], Some("T"))?;
            }
            continue;
        }
        if fields[2] != "function" {
            continue;
        }
        let arity = match fields[1] {
            "SLOT-VALUE" | "SLOT-MAKUNBOUND" | "SLOT-BOUNDP" | "SLOT-EXISTS-P" | "SLOT-UNBOUND" => {
                2
            }
            "SLOT-MISSING" => 4,
            "CLASS-OF" | "CLASS-NAME" => 1,
            _ => 0,
        };
        let callback = match fields[1] {
            "SLOT-MAKUNBOUND" => slot_makunbound_builtin,
            "SLOT-BOUNDP" => slot_boundp_builtin,
            "SLOT-EXISTS-P" => slot_exists_builtin,
            "SLOT-VALUE" => slot_value_builtin,
            "CLASS-OF" => class_of_builtin,
            "CLASS-NAME" => class_name_builtin,
            _ => continue,
        };
        let package = if fields[0] == NCL_MOP {
            BuiltinPackage::NclMop
        } else {
            BuiltinPackage::CommonLisp
        };
        bind(ctx, runtime, package, fields[1], arity, callback)?;
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
    register_owned_symbols(&mut ctx, runtime)?;
    for descriptor in mop::builtin_descriptors() {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(descriptor.package, descriptor.name),
            mop::implementation(*descriptor),
        )?;
    }
    initialization::register_initialization_builtins(runtime)?;
    Ok(())
}

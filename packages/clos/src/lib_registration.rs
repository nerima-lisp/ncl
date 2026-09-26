#[derive(Clone, Copy)]
struct Registration {
    identifier: BuiltinIdentifier,
    implementation: BuiltinImplementation,
}

fn direct_registration(
    package: BuiltinPackage,
    name: &'static str,
    arity: BuiltinArity,
    callback: ncl_object::RustBuiltin,
) -> Registration {
    Registration {
        identifier: BuiltinIdentifier::new(package, BuiltinName::new(name)),
        implementation: BuiltinImplementation::direct(descriptor(arity), callback),
    }
}

fn builtin_manifest() -> Vec<Registration> {
    let mut manifest = vec![
        direct_registration(
            BuiltinPackage::CommonLisp,
            "CLASS-NAME",
            BuiltinArity::One,
            class_name_builtin,
        ),
        direct_registration(
            BuiltinPackage::CommonLisp,
            "CLASS-OF",
            BuiltinArity::One,
            class_of_builtin,
        ),
        direct_registration(
            BuiltinPackage::CommonLisp,
            "SLOT-BOUNDP",
            BuiltinArity::Two,
            slot_boundp_builtin,
        ),
        direct_registration(
            BuiltinPackage::CommonLisp,
            "SLOT-EXISTS-P",
            BuiltinArity::Two,
            slot_exists_builtin,
        ),
        direct_registration(
            BuiltinPackage::CommonLisp,
            "SLOT-MAKUNBOUND",
            BuiltinArity::Two,
            slot_makunbound_builtin,
        ),
        direct_registration(
            BuiltinPackage::CommonLisp,
            "SLOT-VALUE",
            BuiltinArity::Two,
            slot_value_builtin,
        ),
        direct_registration(
            BuiltinPackage::NclMop,
            "CLASS-NAME",
            BuiltinArity::One,
            class_name_builtin,
        ),
    ];
    manifest.extend(mop::builtin_descriptors().iter().map(|descriptor| Registration {
        identifier: BuiltinIdentifier::new(descriptor.package, descriptor.name),
        implementation: mop::implementation(*descriptor),
    }));
    for descriptor in initialization::builtin_descriptors() {
        let implementation = initialization::implementation(*descriptor);
        manifest.push(Registration {
            identifier: BuiltinIdentifier::new(descriptor.package, descriptor.name),
            implementation,
        });
        if descriptor.name.as_str() == "MAKE-INSTANCE" {
            manifest.push(Registration {
                identifier: BuiltinIdentifier::new(BuiltinPackage::NclMop, descriptor.name),
                implementation,
            });
        }
    }
    manifest
}

/// Every callable installed by the CLOS production registration path.
#[must_use]
pub fn production_function_names() -> Vec<BuiltinIdentifier> {
    builtin_manifest()
        .into_iter()
        .map(|registration| registration.identifier)
        .collect()
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
    let manifest = builtin_manifest();
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
                let registration = manifest
                    .iter()
                    .find(|registration| {
                        registration.identifier.package.as_str() == fields[0]
                            && registration.identifier.name.as_str() == fields[1]
                    })
                    .ok_or(ObjectError::TypeError)?;
                let package = runtime.ensure_package(ctx, fields[0])?;
                Package::from_word(package).intern(ctx, runtime, fields[1])?;
                let function = runtime
                    .function(ctx, fields[0], fields[1])
                    .ok_or(ObjectError::TypeError)?;
                let function = ncl_object::FunctionObject::try_from(function)
                    .map_err(|_| ObjectError::TypeError)?;
                if runtime.builtin_descriptor(function) != Some(registration.implementation.descriptor) {
                    return Err(ObjectError::TypeError);
                }
            }
            _ => return Err(ObjectError::TypeError),
        }
    }
    Ok(())
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
    for registration in builtin_manifest() {
        runtime.register_builtin(&mut ctx, registration.identifier, registration.implementation)?;
    }
    let slot_value_set = direct_registration(
        BuiltinPackage::CommonLisp,
        "SLOT-VALUE-SET",
        BuiltinArity::Three,
        slot_set_builtin,
    );
    runtime.register_builtin(&mut ctx, slot_value_set.identifier, slot_value_set.implementation)?;
    register_owned_symbols(&mut ctx, runtime)?;
    Ok(())
}

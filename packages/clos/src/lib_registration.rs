#[derive(Clone, Copy)]
struct Registration {
    identifier: BuiltinIdentifier,
    implementation: BuiltinImplementation,
}

#[derive(Clone, Copy)]
struct DirectBuiltin {
    package: BuiltinPackage,
    name: BuiltinName,
    arity: BuiltinArity,
    callback: ncl_object::RustBuiltin,
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

const DIRECT_BUILTINS: [DirectBuiltin; 8] = [
    DirectBuiltin { package: BuiltinPackage::CommonLisp, name: BuiltinName::new("CLASS-NAME"), arity: BuiltinArity::One, callback: class_name_builtin },
    DirectBuiltin { package: BuiltinPackage::CommonLisp, name: BuiltinName::new("CLASS-OF"), arity: BuiltinArity::One, callback: class_of_builtin },
    DirectBuiltin { package: BuiltinPackage::CommonLisp, name: BuiltinName::new("SLOT-BOUNDP"), arity: BuiltinArity::Two, callback: slot_boundp_builtin },
    DirectBuiltin { package: BuiltinPackage::CommonLisp, name: BuiltinName::new("SLOT-EXISTS-P"), arity: BuiltinArity::Two, callback: slot_exists_builtin },
    DirectBuiltin { package: BuiltinPackage::CommonLisp, name: BuiltinName::new("SLOT-MAKUNBOUND"), arity: BuiltinArity::Two, callback: slot_makunbound_builtin },
    DirectBuiltin { package: BuiltinPackage::CommonLisp, name: BuiltinName::new("SLOT-VALUE"), arity: BuiltinArity::Two, callback: slot_value_builtin },
    DirectBuiltin { package: BuiltinPackage::CommonLisp, name: BuiltinName::new("SLOT-VALUE-SET"), arity: BuiltinArity::Three, callback: slot_set_builtin },
    DirectBuiltin { package: BuiltinPackage::NclMop, name: BuiltinName::new("CLASS-NAME"), arity: BuiltinArity::One, callback: class_name_builtin },
];

fn builtin_manifest() -> Vec<Registration> {
    let mut registrations = DIRECT_BUILTINS
        .iter()
        .map(|builtin| direct_registration(builtin.package, builtin.name.as_str(), builtin.arity, builtin.callback))
        .collect::<Vec<_>>();
    registrations.extend(mop::builtin_descriptors().iter().map(|descriptor| Registration {
        identifier: BuiltinIdentifier::new(descriptor.package, descriptor.name),
        implementation: mop::implementation(*descriptor),
    }));
    for descriptor in initialization::builtin_descriptors() {
        let implementation = initialization::implementation(*descriptor);
        registrations.push(Registration {
            identifier: BuiltinIdentifier::new(descriptor.package, descriptor.name),
            implementation,
        });
        for package in descriptor.aliases {
            registrations.push(Registration {
                identifier: BuiltinIdentifier::new(*package, descriptor.name),
                implementation,
            });
        }
    }
    registrations
}

/// Every callable installed by the CLOS production registration path.
#[must_use]
pub fn production_function_names() -> Vec<BuiltinIdentifier> {
    builtin_manifest()
        .into_iter()
        .map(|registration| registration.identifier)
        .collect()
}

#[derive(Clone, Copy)]
struct ClassDefinition {
    name: BuiltinName,
    superclass: Option<BuiltinName>,
}

const CLASS_NAMES: [BuiltinName; 32] = [
    BuiltinName::new("CLASS"), BuiltinName::new("NULL"), BuiltinName::new("STANDARD-OBJECT"),
    BuiltinName::new("STANDARD-CLASS"), BuiltinName::new("BUILT-IN-CLASS"), BuiltinName::new("STRUCTURE-CLASS"),
    BuiltinName::new("GENERIC-FUNCTION"), BuiltinName::new("STANDARD-GENERIC-FUNCTION"), BuiltinName::new("METHOD"),
    BuiltinName::new("STANDARD-METHOD"), BuiltinName::new("METHOD-COMBINATION"), BuiltinName::new("STRUCTURE-OBJECT"),
    BuiltinName::new("EQL-SPECIALIZER"), BuiltinName::new("T"), BuiltinName::new("NUMBER"), BuiltinName::new("INTEGER"),
    BuiltinName::new("LIST"), BuiltinName::new("CONS"), BuiltinName::new("SYMBOL"), BuiltinName::new("STRING"),
    BuiltinName::new("VECTOR"), BuiltinName::new("ARRAY"), BuiltinName::new("HASH-TABLE"), BuiltinName::new("STREAM"),
    BuiltinName::new("PACKAGE"), BuiltinName::new("FUNCTION"), BuiltinName::new("CHARACTER"), BuiltinName::new("SIMPLE-VECTOR"),
    BuiltinName::new("BIGNUM"), BuiltinName::new("RATIO"), BuiltinName::new("DOUBLE-FLOAT"), BuiltinName::new("COMPLEX"),
];

const fn class(name: &'static str, superclass: Option<&'static str>) -> ClassDefinition {
    ClassDefinition {
        name: BuiltinName::new(name),
        superclass: match superclass {
            Some(name) => Some(BuiltinName::new(name)),
            None => None,
        },
    }
}

const CLASS_DEFINITIONS: [ClassDefinition; 32] = [
    class("T", None), class("NULL", Some("T")), class("CLASS", Some("T")),
    class("STANDARD-OBJECT", Some("T")), class("STANDARD-CLASS", Some("CLASS")),
    class("BUILT-IN-CLASS", Some("CLASS")), class("STRUCTURE-CLASS", Some("CLASS")),
    class("GENERIC-FUNCTION", Some("FUNCTION")), class("STANDARD-GENERIC-FUNCTION", Some("GENERIC-FUNCTION")),
    class("METHOD", Some("STANDARD-OBJECT")), class("STANDARD-METHOD", Some("METHOD")),
    class("METHOD-COMBINATION", Some("STANDARD-OBJECT")), class("STRUCTURE-OBJECT", Some("STANDARD-OBJECT")),
    class("EQL-SPECIALIZER", Some("STANDARD-OBJECT")), class("NUMBER", Some("T")), class("INTEGER", Some("T")),
    class("LIST", Some("T")), class("CONS", Some("T")), class("SYMBOL", Some("T")), class("STRING", Some("T")),
    class("VECTOR", Some("T")), class("ARRAY", Some("T")), class("HASH-TABLE", Some("T")), class("STREAM", Some("T")),
    class("PACKAGE", Some("T")), class("FUNCTION", Some("T")), class("CHARACTER", Some("T")),
    class("SIMPLE-VECTOR", Some("T")), class("BIGNUM", Some("T")), class("RATIO", Some("T")),
    class("DOUBLE-FLOAT", Some("T")), class("COMPLEX", Some("T")),
];

fn register_classes(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, COMMON_LISP)?;
    for name in CLASS_NAMES {
        Package::from_word(package).intern(ctx, runtime, name.as_str())?;
    }
    let [root, null, remaining @ ..] = CLASS_DEFINITIONS;
    install_typed_class(ctx, runtime, root.name, root.superclass)?;
    install_typed_class(ctx, runtime, null.name, null.superclass)?;
    for definition in remaining {
        if runtime.class(ctx, definition.name.as_str()).is_none() {
            install_typed_class(ctx, runtime, definition.name, definition.superclass)?;
        }
    }
    Ok(())
}

fn install_typed_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: BuiltinName,
    superclass: Option<BuiltinName>,
) -> Result<(), ObjectError> {
    let name_word = ncl_object::make_string(ctx, runtime, &name.as_str().chars().collect::<Vec<_>>())?;
    let supers = superclass
        .and_then(|parent| runtime.class(ctx, parent.as_str()))
        .unwrap_or(Word::NIL);
    let class = make_class(ctx, runtime, name_word, supers, Word::NIL, Word::fixnum(0))?;
    runtime.define_class(ctx, name.as_str(), class)
}

/// Register the typed CLOS class and builtin tables.
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
    Ok(())
}

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

const MACRO_FORM: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("FORM"),
    ty: ncl_object::ParameterType::Any,
};
const MACRO_ENV: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ENV"),
    ty: ncl_object::ParameterType::Any,
};
const STRUCTURE_REST: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("SLOT"),
    ty: ncl_object::ParameterType::Any,
};

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

const DIRECT_BUILTINS: [DirectBuiltin; 23] = [
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("CLASS-NAME"),
        arity: BuiltinArity::One,
        callback: class_name_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("CLASS-OF"),
        arity: BuiltinArity::One,
        callback: class_of_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("FIND-CLASS"),
        arity: BuiltinArity::One,
        callback: find_class_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("SLOT-BOUNDP"),
        arity: BuiltinArity::Two,
        callback: slot_boundp_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("SLOT-EXISTS-P"),
        arity: BuiltinArity::Two,
        callback: slot_exists_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("SLOT-MAKUNBOUND"),
        arity: BuiltinArity::Two,
        callback: slot_makunbound_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("SLOT-VALUE"),
        arity: BuiltinArity::Two,
        callback: slot_value_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("SLOT-VALUE-SET"),
        arity: BuiltinArity::Three,
        callback: slot_set_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("TYPEP"),
        arity: BuiltinArity::Two,
        callback: typep_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%STRUCTURE-REF"),
        arity: BuiltinArity::Two,
        callback: structure_ref_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%STRUCTURE-SET"),
        arity: BuiltinArity::Three,
        callback: structure_set_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%STRUCTURE-P"),
        arity: BuiltinArity::Two,
        callback: structure_predicate_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%STRUCTURE-COPY"),
        arity: BuiltinArity::One,
        callback: structure_copy_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-DEFINE-GENERIC"),
        arity: BuiltinArity::Two,
        callback: clos_define_generic_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-REDEFINE-GENERIC"),
        arity: BuiltinArity::Two,
        callback: clos_redefine_generic_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-GENERIC-FUNCTION-REGISTERED-P"),
        arity: BuiltinArity::One,
        callback: clos_generic_function_registered_p_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-DEFGENERIC-ERROR"),
        arity: BuiltinArity::One,
        callback: defgeneric_program_error_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-ADD-METHOD"),
        arity: BuiltinArity::Three,
        callback: clos_add_method_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-ENSURE-INITIALIZATION-BASE"),
        arity: BuiltinArity::One,
        callback: clos_ensure_initialization_base_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-DISPATCH"),
        arity: BuiltinArity::Two,
        callback: clos_dispatch_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-CALL-NEXT-METHOD"),
        arity: BuiltinArity::Two,
        callback: clos_call_next_method_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("%CLOS-NEXT-METHOD-P"),
        arity: BuiltinArity::One,
        callback: clos_next_method_p_builtin,
    },
    DirectBuiltin {
        package: BuiltinPackage::NclMop,
        name: BuiltinName::new("CLASS-NAME"),
        arity: BuiltinArity::One,
        callback: class_name_builtin,
    },
];

fn builtin_manifest() -> Vec<Registration> {
    let mut registrations = DIRECT_BUILTINS
        .iter()
        .map(|builtin| {
            direct_registration(
                builtin.package,
                builtin.name.as_str(),
                builtin.arity,
                builtin.callback,
            )
        })
        .collect::<Vec<_>>();
    registrations.extend(
        mop::builtin_descriptors()
            .iter()
            .map(|descriptor| Registration {
                identifier: BuiltinIdentifier::new(descriptor.package, descriptor.name),
                implementation: mop::implementation(*descriptor),
            }),
    );
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
    let macro_builtin = Builtin {
        lambda_list: LambdaList::with_rest(&[MACRO_FORM], MACRO_ENV),
        convention: ncl_object::BuiltinConvention::Adapted,
    };
    let macro_registration = |name, callback| Registration {
        identifier: BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
        implementation: BuiltinImplementation::adapted(macro_builtin, callback, |args| {
            Ok((0..args.len())
                .filter_map(|index| args.get(index))
                .collect())
        }),
    };
    registrations.push(Registration {
        identifier: BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("FIND-METHOD"),
        ),
        implementation: BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_optional(FIND_METHOD_ARGUMENTS, &[ERRORP_ARGUMENT]),
                convention: ncl_object::BuiltinConvention::Adapted,
            },
            find_method_builtin,
            |args| Ok(args.as_slice().to_vec()),
        ),
    });
    registrations.push(macro_registration("DEFCLASS", defclass_macro_builtin));
    registrations.push(macro_registration("DEFGENERIC", defgeneric_macro_builtin));
    registrations.push(macro_registration("DEFMETHOD", defmethod_macro_builtin));
    registrations.push(Registration {
        identifier: BuiltinIdentifier::new(
            BuiltinPackage::CommonLisp,
            BuiltinName::new("%STRUCTURE-MAKE"),
        ),
        implementation: BuiltinImplementation::adapted(
            Builtin {
                lambda_list: LambdaList::with_rest(&[MACRO_FORM], STRUCTURE_REST),
                convention: ncl_object::BuiltinConvention::Adapted,
            },
            structure_make_builtin,
            |args| {
                Ok((0..args.len())
                    .filter_map(|index| args.get(index))
                    .collect())
            },
        ),
    });
    registrations.push(macro_registration("DEFSTRUCT", defstruct_macro_builtin));
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
    secondary_superclass: Option<BuiltinName>,
    kind: ClassKind,
}

#[derive(Clone, Copy)]
enum ClassKind {
    BuiltIn,
    Standard,
}

impl ClassKind {
    const fn word(self) -> Word {
        match self {
            Self::BuiltIn => Word::fixnum(0),
            Self::Standard => Word::NIL,
        }
    }
}

const fn class(name: &'static str, superclass: Option<&'static str>) -> ClassDefinition {
    ClassDefinition {
        name: BuiltinName::new(name),
        superclass: match superclass {
            Some(name) => Some(BuiltinName::new(name)),
            None => None,
        },
        secondary_superclass: None,
        kind: ClassKind::BuiltIn,
    }
}

const fn class_with_superclasses(
    name: &'static str,
    superclass: &'static str,
    secondary_superclass: &'static str,
) -> ClassDefinition {
    ClassDefinition {
        name: BuiltinName::new(name),
        superclass: Some(BuiltinName::new(superclass)),
        secondary_superclass: Some(BuiltinName::new(secondary_superclass)),
        kind: ClassKind::BuiltIn,
    }
}

const fn standard_class(name: &'static str, superclass: Option<&'static str>) -> ClassDefinition {
    ClassDefinition {
        name: BuiltinName::new(name),
        superclass: match superclass {
            Some(name) => Some(BuiltinName::new(name)),
            None => None,
        },
        secondary_superclass: None,
        kind: ClassKind::Standard,
    }
}

const CLASS_DEFINITIONS: &[ClassDefinition] = &[
    class("T", None),
    standard_class("STANDARD-OBJECT", Some("T")),
    standard_class("CLASS", Some("STANDARD-OBJECT")),
    standard_class("STANDARD-CLASS", Some("CLASS")),
    standard_class("BUILT-IN-CLASS", Some("CLASS")),
    standard_class("STRUCTURE-CLASS", Some("CLASS")),
    class("NUMBER", Some("T")),
    class("REAL", Some("NUMBER")),
    class("RATIONAL", Some("REAL")),
    class("INTEGER", Some("RATIONAL")),
    class("BIGNUM", Some("INTEGER")),
    class("RATIO", Some("RATIONAL")),
    class("FLOAT", Some("REAL")),
    class("SHORT-FLOAT", Some("FLOAT")),
    class("SINGLE-FLOAT", Some("FLOAT")),
    class("DOUBLE-FLOAT", Some("FLOAT")),
    class("LONG-FLOAT", Some("FLOAT")),
    class("COMPLEX", Some("NUMBER")),
    class("SEQUENCE", Some("T")),
    class("LIST", Some("SEQUENCE")),
    class("CONS", Some("LIST")),
    class("SYMBOL", Some("T")),
    class_with_superclasses("NULL", "SYMBOL", "LIST"),
    class("ARRAY", Some("T")),
    class_with_superclasses("VECTOR", "ARRAY", "SEQUENCE"),
    class("BIT-VECTOR", Some("VECTOR")),
    class("SIMPLE-BIT-VECTOR", Some("BIT-VECTOR")),
    class("SIMPLE-ARRAY", Some("ARRAY")),
    class("STRING", Some("VECTOR")),
    class("BASE-STRING", Some("STRING")),
    class("SIMPLE-STRING", Some("STRING")),
    class_with_superclasses("SIMPLE-BASE-STRING", "SIMPLE-STRING", "BASE-STRING"),
    class("SIMPLE-VECTOR", Some("VECTOR")),
    class("FUNCTION", Some("T")),
    class("COMPILED-FUNCTION", Some("FUNCTION")),
    standard_class("GENERIC-FUNCTION", Some("FUNCTION")),
    standard_class("STANDARD-GENERIC-FUNCTION", Some("GENERIC-FUNCTION")),
    standard_class("METHOD", Some("STANDARD-OBJECT")),
    standard_class("STANDARD-METHOD", Some("METHOD")),
    standard_class("METHOD-COMBINATION", Some("STANDARD-OBJECT")),
    class("STRUCTURE-OBJECT", Some("T")),
    standard_class("EQL-SPECIALIZER", Some("STANDARD-OBJECT")),
    class("HASH-TABLE", Some("T")),
    class("STREAM", Some("T")),
    class("BROADCAST-STREAM", Some("STREAM")),
    class("CONCATENATED-STREAM", Some("STREAM")),
    class("ECHO-STREAM", Some("STREAM")),
    class("FILE-STREAM", Some("STREAM")),
    class("STRING-STREAM", Some("STREAM")),
    class("SYNONYM-STREAM", Some("STREAM")),
    class("TWO-WAY-STREAM", Some("STREAM")),
    class("PACKAGE", Some("T")),
    class("PATHNAME", Some("T")),
    class("LOGICAL-PATHNAME", Some("PATHNAME")),
    class("CHARACTER", Some("T")),
    class("RANDOM-STATE", Some("T")),
    class("READTABLE", Some("T")),
    class("RESTART", Some("T")),
];

fn register_classes(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let package = runtime.ensure_package(ctx, COMMON_LISP)?;
    for definition in CLASS_DEFINITIONS {
        Package::from_word(package).intern(ctx, runtime, definition.name.as_str())?;
    }
    for definition in CLASS_DEFINITIONS.iter().copied() {
        let must_install = matches!(definition.name.as_str(), "T" | "NULL");
        // The ownership table and `ncl-types` register placeholders before
        // this crate runs. Neither `Word::UNBOUND` nor `Word::fixnum(1)` is a
        // class descriptor, so both must count as not installed.
        if must_install || !is_installed_class(ctx, runtime, definition.name.as_str()) {
            install_typed_class(
                ctx,
                runtime,
                definition.name,
                definition.superclass,
                definition.secondary_superclass,
                definition.kind,
            )?;
        }
    }
    Ok(())
}

/// Whether `name` already names a *real* (non-placeholder) class.
fn is_installed_class(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> bool {
    matches!(runtime.class(ctx, name), Some(word) if word != Word::UNBOUND && word != Word::fixnum(1)) // check-added-lines: allow(unbound) ownership-table placeholder
}

fn install_typed_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: BuiltinName,
    superclass: Option<BuiltinName>,
    secondary_superclass: Option<BuiltinName>,
    kind: ClassKind,
) -> Result<(), ObjectError> {
    let mut scope = Scope::new(ctx);
    let name_word = scope.make_string(runtime, &name.as_str().chars().collect::<Vec<_>>())?;
    // A superclass may exist only as the ownership table's `Word::UNBOUND`
    // placeholder (see `is_installed_class`); treat that the same as "not
    // registered yet" rather than passing the sentinel through as if it
    // were a real superclass object.
    let mut supers = scope.root_many(&[]);
    for parent in superclass.into_iter().chain(secondary_superclass) {
        if let Some(word) = runtime
            .class(scope.context_mut(), parent.as_str())
            // check-added-lines: allow(unbound) absent classes use this marker
            .filter(|word| *word != Word::UNBOUND && *word != Word::fixnum(1))
        {
            supers.push(&mut scope, Local::from_word(word));
        }
    }
    let superclass = match supers.len() {
        0 => scope.root(Local::from_word(Word::NIL)),
        1 => {
            let parent_handle = *supers.as_slice().first().ok_or(ObjectError::Layout)?;
            let parent = scope.get(parent_handle).as_word();
            scope.root(Local::from_word(parent))
        }
        2.. => scope.make_list(runtime, &supers)?,
    };
    let name_word = scope.get(name_word).as_word();
    let superclass_word = scope.get(superclass).as_word();
    let class = make_class(
        scope.context_mut(),
        runtime,
        name_word,
        superclass_word,
        Word::NIL,
        kind.word(),
    )?;
    runtime.define_class(scope.context_mut(), name.as_str(), class)
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
    ncl_conditions::wire_condition_root_to_t(&mut ctx, runtime)?;
    for registration in builtin_manifest() {
        runtime.register_builtin(
            &mut ctx,
            registration.identifier,
            registration.implementation,
        )?;
    }
    Ok(())
}

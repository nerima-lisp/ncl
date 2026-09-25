//! CLOS class descriptors and the NCL-MOP registration boundary.

pub mod domain;
pub mod initialization;
pub mod mop;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, Fixnum, Instance, LambdaList, MultipleValues, ObjectError, ObjectRef,
    ObjectType, Package, Runtime, ThreadContext, Word, classify_object, instance_class,
    make_instance as allocate_instance, make_simple_vector, simple_vector_length,
    simple_vector_ref, slot_ref, slot_set,
};

const COMMON_LISP: &str = "COMMON-LISP";
const NCL_MOP: &str = "NCL-MOP";
const CLASS_NAME: usize = 0;
const CLASS_DIRECT_SUPERCLASS: usize = 1;
const CLASS_SLOTS: usize = 2;

const ARGUMENT: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARG"),
    ty: ncl_object::ParameterType::Any,
};
const ARGS_0: &[ncl_object::Parameter] = &[];
const ARGS_1: &[ncl_object::Parameter] = &[ARGUMENT];
const ARGS_2: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT];
const ARGS_3: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT, ARGUMENT];
const ARGS_4: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT, ARGUMENT, ARGUMENT];

const fn descriptor(arity: u8) -> Builtin {
    let required = match arity {
        0 => ARGS_0,
        1 => ARGS_1,
        2 => ARGS_2,
        3 => ARGS_3,
        4 => ARGS_4,
        _ => &[],
    };
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: ncl_object::BuiltinConvention::Direct(Arity::exact(arity)),
    }
}

/// Allocate a class descriptor backed by an object-layer vector.
///
/// # Errors
///
/// Returns an object allocation or storage error.
pub fn make_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    direct_superclasses: Word,
    slots: Word,
    kind: Word,
) -> Result<Word, ObjectError> {
    make_simple_vector(ctx, runtime, &[name, direct_superclasses, slots, kind])
}

/// Return the name stored in a class descriptor.
///
/// # Errors
///
/// Returns an object layout or storage error when `class` is not a vector.
pub fn class_name(ctx: &ThreadContext, class: Word) -> Result<Word, ObjectError> {
    simple_vector_ref(ctx, class, CLASS_NAME)
}

/// Return the class of an object using the existing object-layer layouts.
///
/// # Errors
///
/// Returns an object layout error when the corresponding built-in class is absent.
pub fn class_of(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    if let Ok(class) = instance_class(ctx, Instance::from_word(object)) {
        return Ok(class);
    }
    let name = if object == Word::NIL {
        "NULL"
    } else {
        match classify_object(ctx, object) {
            ObjectRef::Fixnum(_) => "INTEGER",
            ObjectRef::Character(_) => "CHARACTER",
            ObjectRef::Cons(_) => "CONS",
            ObjectRef::Symbol(_) => "SYMBOL",
            ObjectRef::String(_) => "STRING",
            ObjectRef::SimpleVector(_) => "SIMPLE-VECTOR",
            ObjectRef::Array(_) | ObjectRef::SpecializedArray(_) => "ARRAY",
            ObjectRef::HashTable(_) => "HASH-TABLE",
            ObjectRef::Function(_) | ObjectRef::Closure(_) => "FUNCTION",
            ObjectRef::Package(_) => "PACKAGE",
            ObjectRef::Stream(_) => "STREAM",
            ObjectRef::Structure(_) => "STRUCTURE-OBJECT",
            ObjectRef::Bignum(_) => "BIGNUM",
            ObjectRef::Ratio(_) => "RATIO",
            ObjectRef::DoubleFloat(_) => "DOUBLE-FLOAT",
            ObjectRef::Complex(_) => "COMPLEX",
            _ => "T",
        }
    };
    runtime
        .class(ctx, name)
        .or_else(|| runtime.class(ctx, "T"))
        .ok_or(ObjectError::Layout)
}

/// Allocate an instance with an already finalized slot vector.
///
/// # Errors
///
/// Returns an object allocation or storage error.
pub fn make_instance(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: Word,
    slots: &[Word],
) -> Result<Word, ObjectError> {
    Ok(allocate_instance(ctx, runtime, class, slots)?.as_word())
}

const fn typed_error(ctx: &mut ThreadContext, error: ncl_object::LispError) -> ObjectError {
    ctx.set_pending_lisp_error(error);
    ObjectError::TypeError
}

fn instance_arg(ctx: &mut ThreadContext, word: Word) -> Result<Instance, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Instance(_) => Ok(Instance::from_word(word)),
        _ => Err(typed_error(
            ctx,
            ncl_object::LispError::TypeError {
                datum: word,
                expected: ObjectType::Instance,
            },
        )),
    }
}

fn fixnum_arg(ctx: &mut ThreadContext, word: Word) -> Result<Fixnum, ObjectError> {
    Fixnum::try_from_word(word).map_err(|error| typed_error(ctx, error.into()))
}

fn slot_value_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let index = fixnum_arg(ctx, args.required(1)?)?;
    slot_ref(
        ctx,
        instance,
        usize::try_from(index.value()).map_err(|_| ObjectError::TypeError)?,
    )
}

fn slot_set_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let index = fixnum_arg(ctx, args.required(1)?)?;
    let value = args.required(2)?;
    slot_set(
        ctx,
        instance,
        usize::try_from(index.value()).map_err(|_| ObjectError::TypeError)?,
        value,
    )?;
    Ok(value)
}

fn slot_boundp_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = slot_value_builtin(ctx, runtime, args, values)?;
    Ok(if value == Word::UNBOUND {
        Word::NIL
    } else {
        Word::TRUE
    })
}

fn slot_makunbound_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let index = fixnum_arg(ctx, args.required(1)?)?;
    slot_set(
        ctx,
        instance,
        usize::try_from(index.value()).map_err(|_| ObjectError::TypeError)?,
        Word::UNBOUND,
    )?;
    Ok(instance.as_word())
}

fn slot_exists_in_class(
    ctx: &ThreadContext,
    class: Word,
    slot_name: Word,
) -> Result<bool, ObjectError> {
    let slots = simple_vector_ref(ctx, class, CLASS_SLOTS)?;
    if slots != Word::NIL {
        for index in 0..simple_vector_length(ctx, slots)? {
            let slot = simple_vector_ref(ctx, slots, index)?;
            let name = match simple_vector_length(ctx, slot) {
                Ok(length) if length >= 2 => simple_vector_ref(ctx, slot, 0)?,
                _ => slot,
            };
            if name == slot_name {
                return Ok(true);
            }
        }
    }
    let superclass = simple_vector_ref(ctx, class, CLASS_DIRECT_SUPERCLASS)?;
    if superclass == Word::NIL {
        return Ok(false);
    }
    slot_exists_in_class(ctx, superclass, slot_name)
}

fn slot_exists_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let class = instance_class(ctx, instance)?;
    let slot_name = args.required(1)?;
    Ok(if slot_exists_in_class(ctx, class, slot_name)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn class_of_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_of(ctx, runtime, args.required(0)?)
}

fn class_name_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_name(ctx, args.required(0)?)
}

fn install_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    superclass: Option<&str>,
) -> Result<(), ObjectError> {
    let name_word = ncl_object::make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    let supers = superclass
        .and_then(|parent| runtime.class(ctx, parent))
        .unwrap_or(Word::NIL);
    let class = make_class(ctx, runtime, name_word, supers, Word::NIL, Word::fixnum(0))?;
    runtime.define_class(ctx, name, class)
}

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

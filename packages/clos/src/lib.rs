//! CLOS class descriptors and the NCL-MOP registration boundary.

use ncl_object::{
    Builtin, BuiltinImplementation, ObjectError, Package, Runtime, ThreadContext, Word,
    classify_object, instance_class, make_instance as allocate_instance, make_simple_vector,
    simple_vector_ref, slot_ref, slot_set,
};

const COMMON_LISP: &str = "COMMON-LISP";
const NCL_MOP: &str = "NCL-MOP";
const CLASS_NAME: usize = 0;

/// Allocate a class descriptor backed by an object-layer vector.
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
pub fn class_name(ctx: &ThreadContext, class: Word) -> Result<Word, ObjectError> {
    simple_vector_ref(ctx, class, CLASS_NAME)
}

/// Return the class of an object using the existing object-layer layouts.
pub fn class_of(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    if let Ok(class) = instance_class(ctx, ncl_object::Instance::from(object)) {
        return Ok(class);
    }
    let name = match classify_object(ctx, object) {
        ncl_object::ObjectRef::Fixnum(_) => "INTEGER",
        ncl_object::ObjectRef::Character(_) => "CHARACTER",
        ncl_object::ObjectRef::Cons(_) => "CONS",
        ncl_object::ObjectRef::Symbol(_) => "SYMBOL",
        ncl_object::ObjectRef::String(_) => "STRING",
        ncl_object::ObjectRef::SimpleVector(_) => "SIMPLE-VECTOR",
        ncl_object::ObjectRef::Array(_) | ncl_object::ObjectRef::SpecializedArray(_) => "ARRAY",
        ncl_object::ObjectRef::HashTable(_) => "HASH-TABLE",
        ncl_object::ObjectRef::Function(_) | ncl_object::ObjectRef::Closure(_) => "FUNCTION",
        ncl_object::ObjectRef::Package(_) => "PACKAGE",
        ncl_object::ObjectRef::Stream(_) => "STREAM",
        ncl_object::ObjectRef::Structure(_) => "STRUCTURE-OBJECT",
        ncl_object::ObjectRef::Bignum(_) => "BIGNUM",
        ncl_object::ObjectRef::Ratio(_) => "RATIO",
        ncl_object::ObjectRef::DoubleFloat(_) => "DOUBLE-FLOAT",
        ncl_object::ObjectRef::Complex(_) => "COMPLEX",
        _ => "T",
    };
    runtime
        .class(ctx, name)
        .or_else(|| runtime.class(ctx, "T"))
        .ok_or(ObjectError::Layout)
}

/// Allocate an instance with an already finalized slot vector.
pub fn make_instance(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: Word,
    slots: &[Word],
) -> Result<Word, ObjectError> {
    Ok(allocate_instance(ctx, runtime, class, slots)?.as_word())
}

fn unsupported(
    _ctx: &mut ThreadContext,
    _args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Err(ObjectError::Unbound)
}

fn slot_value_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let index = args
        .get(1)
        .and_then(|word| word.as_fixnum())
        .ok_or(ObjectError::TypeError)?;
    slot_ref(
        ctx,
        ncl_object::Instance::from(args[0]),
        usize::try_from(index).map_err(|_| ObjectError::TypeError)?,
    )
}

fn slot_set_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let index = args
        .get(1)
        .and_then(|word| word.as_fixnum())
        .ok_or(ObjectError::TypeError)?;
    let index = usize::try_from(index).map_err(|_| ObjectError::TypeError)?;
    slot_set(ctx, ncl_object::Instance::from(args[0]), index, args[2])?;
    Ok(args[2])
}

fn slot_boundp_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let value = slot_value_builtin(ctx, args, values)?;
    Ok(if value == Word::UNBOUND {
        Word::NIL
    } else {
        Word::TRUE
    })
}

fn slot_makunbound_builtin(
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    let index = args
        .get(1)
        .and_then(|word| word.as_fixnum())
        .ok_or(ObjectError::TypeError)?;
    let index = usize::try_from(index).map_err(|_| ObjectError::TypeError)?;
    slot_set(
        ctx,
        ncl_object::Instance::from(args[0]),
        index,
        Word::UNBOUND,
    )?;
    Ok(args[0])
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
    package: &str,
    name: &str,
    arity: u8,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        package,
        name,
        BuiltinImplementation::direct(
            Builtin {
                arity,
                direct: true,
                lambda_list: "",
            },
            function,
        ),
    )?;
    Ok(())
}

/// Register CLOS classes, NCL-MOP names, and the implemented slot builtins.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    runtime.ensure_package(&mut ctx, NCL_MOP)?;
    for name in [
        "CLASS",
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
        let package = runtime.ensure_package(&mut ctx, COMMON_LISP)?;
        Package::from(package).intern(&mut ctx, runtime, name)?;
    }
    install_class(&mut ctx, runtime, "T", None)?;
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
        if runtime.class(&mut ctx, name).is_none() {
            install_class(&mut ctx, runtime, name, superclass)?;
        }
    }
    for row in include_str!("../ownership.tsv").lines().skip(1) {
        let fields: Vec<_> = row.split('\t').collect();
        if fields.len() < 3 {
            continue;
        }
        let package = runtime.ensure_package(&mut ctx, fields[0])?;
        Package::from(package).intern(&mut ctx, runtime, fields[1])?;
        if fields[2] == "class" {
            if runtime.class(&mut ctx, fields[1]).is_none() {
                install_class(&mut ctx, runtime, fields[1], Some("T"))?;
            }
            continue;
        }
        if fields[2] != "function" {
            continue;
        }
        let arity = match fields[1] {
            "SLOT-VALUE" | "SLOT-MAKUNBOUND" | "SLOT-BOUNDP" | "SLOT-EXISTS-P" => 2,
            "SLOT-MISSING" => 4,
            "SLOT-UNBOUND" => 2,
            "CLASS-OF" => 1,
            _ => 0,
        };
        let callback = match fields[1] {
            "SLOT-MAKUNBOUND" => slot_makunbound_builtin,
            "SLOT-BOUNDP" => slot_boundp_builtin,
            "SLOT-VALUE" | "SLOT-EXISTS-P" | "SLOT-UNBOUND" => slot_value_builtin,
            _ => unsupported,
        };
        bind(&mut ctx, runtime, fields[0], fields[1], arity, callback)?;
    }
    bind(
        &mut ctx,
        runtime,
        COMMON_LISP,
        "SLOT-VALUE-SET",
        3,
        slot_set_builtin,
    )
}

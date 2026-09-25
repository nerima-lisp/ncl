//! NCL-MOP callbacks for the object-backed CLOS descriptors.
//!
//! This module is deliberately separate from registration.  The runtime owner
//! can expose [`builtin_descriptors`] from its registration pass without
//! changing the descriptor representation used by `ncl-clos` today.

use ncl_object::{
    classify_object, instance_class, make_simple_vector, simple_vector_length, simple_vector_ref,
    slot_ref, slot_set, Arity, Builtin, BuiltinArgs, BuiltinImplementation, BuiltinName,
    BuiltinPackage, Fixnum, Instance, LambdaList, MultipleValues, ObjectError, ObjectRef, Runtime,
    ThreadContext, Word,
};

const ARG: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARG"),
    ty: ncl_object::ParameterType::Any,
};
const A1: &[ncl_object::Parameter] = &[ARG];
const A3: &[ncl_object::Parameter] = &[ARG, ARG, ARG];

const fn fixed(required: &'static [ncl_object::Parameter]) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: ncl_object::BuiltinConvention::Direct(Arity::exact(required.len() as u8)),
    }
}

/// A registration-ready MOP callback descriptor.
#[derive(Clone, Copy, Debug)]
pub struct MopBuiltinDescriptor {
    /// Package containing the function name.
    pub package: BuiltinPackage,
    /// External Lisp name.
    pub name: BuiltinName,
    /// Typed argument and calling convention metadata.
    pub builtin: Builtin,
    /// Safe Rust callback used by `Runtime::register_builtin`.
    pub callback: ncl_object::RustBuiltin,
}

/// Return the MOP callbacks owned by this module.
#[must_use]
pub const fn builtin_descriptors() -> &'static [MopBuiltinDescriptor] {
    &BUILTINS
}

const BUILTINS: [MopBuiltinDescriptor; 9] = [
    descriptor(
        "CLASS-PRECEDENCE-LIST",
        fixed(A1),
        class_precedence_list_builtin,
    ),
    descriptor("CLASS-SLOTS", fixed(A1), class_slots_builtin),
    descriptor("CLASS-DIRECT-SLOTS", fixed(A1), class_direct_slots_builtin),
    descriptor(
        "SLOT-DEFINITION-NAME",
        fixed(A1),
        slot_definition_name_builtin,
    ),
    descriptor(
        "SLOT-DEFINITION-LOCATION",
        fixed(A1),
        slot_definition_location_builtin,
    ),
    descriptor(
        "SLOT-VALUE-USING-CLASS",
        fixed(A3),
        slot_value_using_class_builtin,
    ),
    descriptor(
        "SLOT-BOUNDP-USING-CLASS",
        fixed(A3),
        slot_boundp_using_class_builtin,
    ),
    descriptor(
        "SLOT-MAKUNBOUND-USING-CLASS",
        fixed(A3),
        slot_makunbound_using_class_builtin,
    ),
    descriptor(
        "EQL-SPECIALIZER-OBJECT",
        fixed(A1),
        eql_specializer_object_builtin,
    ),
];

const fn descriptor(
    name: &'static str,
    builtin: Builtin,
    callback: ncl_object::RustBuiltin,
) -> MopBuiltinDescriptor {
    MopBuiltinDescriptor {
        package: BuiltinPackage::NclMop,
        name: BuiltinName::new(name),
        builtin,
        callback,
    }
}

/// Class descriptor field containing its direct superclass descriptor.
pub const CLASS_DIRECT_SUPERCLASS: usize = 1;
/// Class descriptor field containing its effective slot descriptor vector.
pub const CLASS_SLOTS: usize = 2;
/// Slot descriptor field containing its name.
pub const SLOT_NAME: usize = 0;
/// Slot descriptor field containing its finalized instance location.
pub const SLOT_LOCATION: usize = 1;
/// EQL-specializer field containing the specialized object.
pub const EQL_SPECIALIZER_OBJECT: usize = 1;

/// Make a slot descriptor understood by the callbacks in this module.
pub fn make_slot_descriptor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    location: Option<Fixnum>,
) -> Result<Word, ObjectError> {
    make_simple_vector(
        ctx,
        runtime,
        &[name, location.map_or(Word::NIL, Fixnum::as_word)],
    )
}

/// Make an EQL-specializer descriptor understood by this module.
pub fn make_eql_specializer(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    make_simple_vector(ctx, runtime, &[Word::fixnum(1), object])
}

fn vector_field(ctx: &ThreadContext, object: Word, field: usize) -> Result<Word, ObjectError> {
    simple_vector_ref(ctx, object, field)
}

fn class_field(ctx: &ThreadContext, class: Word, field: usize) -> Result<Word, ObjectError> {
    vector_field(ctx, class, field)
}

fn slot_field(ctx: &ThreadContext, slot: Word, field: usize) -> Result<Word, ObjectError> {
    vector_field(ctx, slot, field)
}

fn instance_arg(ctx: &mut ThreadContext, object: Word) -> Result<Instance, ObjectError> {
    match classify_object(ctx, object) {
        ObjectRef::Instance(_) => Ok(Instance::from_word(object)),
        _ => Err(ObjectError::TypeError),
    }
}

fn location_arg(ctx: &ThreadContext, slot: Word) -> Result<usize, ObjectError> {
    let location = slot_field(ctx, slot, SLOT_LOCATION)?;
    let fixnum = Fixnum::try_from_word(location).map_err(|_| ObjectError::TypeError)?;
    usize::try_from(fixnum.value()).map_err(|_| ObjectError::TypeError)
}

fn class_precedence_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: Word,
) -> Result<Word, ObjectError> {
    let mut result = Vec::new();
    let mut current = class;
    for _ in 0..=simple_vector_length(ctx, class)? {
        result.push(current);
        let parent = class_field(ctx, current, CLASS_DIRECT_SUPERCLASS)?;
        if parent == Word::NIL {
            break;
        }
        current = parent;
    }
    make_simple_vector(ctx, runtime, &result)
}

fn class_precedence_list_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_precedence_list(ctx, runtime, args.required(0)?)
}

fn class_slots_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_field(ctx, args.required(0)?, CLASS_SLOTS)
}

fn class_direct_slots_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_field(ctx, args.required(0)?, CLASS_SLOTS)
}

fn slot_definition_name_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    slot_field(ctx, args.required(0)?, SLOT_NAME)
}

fn slot_definition_location_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    slot_field(ctx, args.required(0)?, SLOT_LOCATION)
}

fn checked_instance_for_class(
    ctx: &mut ThreadContext,
    class: Word,
    object: Word,
) -> Result<Instance, ObjectError> {
    let instance = instance_arg(ctx, object)?;
    if instance_class(ctx, instance)? != class {
        return Err(ObjectError::TypeError);
    }
    Ok(instance)
}

fn slot_value_using_class_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let class = args.required(0)?;
    let instance = checked_instance_for_class(ctx, class, args.required(1)?)?;
    let location = location_arg(ctx, args.required(2)?)?;
    slot_ref(ctx, instance, location)
}

fn slot_boundp_using_class_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let class = args.required(0)?;
    let instance = checked_instance_for_class(ctx, class, args.required(1)?)?;
    let location = location_arg(ctx, args.required(2)?)?;
    let value = slot_ref(ctx, instance, location)?;
    Ok(if value == Word::UNBOUND {
        Word::NIL
    } else {
        Word::TRUE
    })
}

fn slot_makunbound_using_class_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let class = args.required(0)?;
    let instance = checked_instance_for_class(ctx, class, args.required(1)?)?;
    let location = location_arg(ctx, args.required(2)?)?;
    slot_set(ctx, instance, location, Word::UNBOUND)?;
    Ok(instance.as_word())
}

fn eql_specializer_object_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    vector_field(ctx, args.required(0)?, EQL_SPECIALIZER_OBJECT)
}

/// Build the implementation passed to `Runtime::register_builtin`.
#[must_use]
pub fn implementation(descriptor: MopBuiltinDescriptor) -> BuiltinImplementation {
    BuiltinImplementation::direct(descriptor.builtin, descriptor.callback)
}

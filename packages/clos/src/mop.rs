//! NCL-MOP callbacks for the object-backed CLOS descriptors.
//!
//! This module is deliberately separate from registration.  The runtime owner
//! can expose [`builtin_descriptors`] from its registration pass without
//! changing the descriptor representation used by `ncl-clos` today.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinImplementation, BuiltinName, BuiltinPackage, Fixnum,
    HandleVec, Instance, LambdaList, Local, MultipleValues, ObjectError, ObjectRef, Runtime, Scope,
    ThreadContext, Word, car, classify_object, instance_class, simple_vector_length,
    simple_vector_ref, slot_ref, slot_set,
};

const ARG: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARG"),
    ty: ncl_object::ParameterType::Any,
};
const A1: &[ncl_object::Parameter] = &[ARG];
const A3: &[ncl_object::Parameter] = &[ARG, ARG, ARG];

const fn fixed(required: &'static [ncl_object::Parameter], arity: Arity) -> Builtin {
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: ncl_object::BuiltinConvention::Direct(arity),
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
        fixed(A1, Arity::exact(1)),
        class_precedence_list_builtin,
    ),
    descriptor(
        "CLASS-SLOTS",
        fixed(A1, Arity::exact(1)),
        class_slots_builtin,
    ),
    descriptor(
        "CLASS-DIRECT-SLOTS",
        fixed(A1, Arity::exact(1)),
        class_direct_slots_builtin,
    ),
    descriptor(
        "SLOT-DEFINITION-NAME",
        fixed(A1, Arity::exact(1)),
        slot_definition_name_builtin,
    ),
    descriptor(
        "SLOT-DEFINITION-LOCATION",
        fixed(A1, Arity::exact(1)),
        slot_definition_location_builtin,
    ),
    descriptor(
        "SLOT-VALUE-USING-CLASS",
        fixed(A3, Arity::exact(3)),
        slot_value_using_class_builtin,
    ),
    descriptor(
        "SLOT-BOUNDP-USING-CLASS",
        fixed(A3, Arity::exact(3)),
        slot_boundp_using_class_builtin,
    ),
    descriptor(
        "SLOT-MAKUNBOUND-USING-CLASS",
        fixed(A3, Arity::exact(3)),
        slot_makunbound_using_class_builtin,
    ),
    descriptor(
        "EQL-SPECIALIZER-OBJECT",
        fixed(A1, Arity::exact(1)),
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
/// Class descriptor field containing effective slot metadata.
pub const CLASS_EFFECTIVE_SLOTS: usize = 4;
/// Slot descriptor field containing its name.
pub const SLOT_NAME: usize = 0;
/// Slot descriptor field containing its finalized instance location.
pub const SLOT_LOCATION: usize = 1;
/// EQL-specializer field containing the specialized object.
pub const EQL_SPECIALIZER_OBJECT: usize = 1;

/// Make a slot descriptor understood by the callbacks in this module.
///
/// # Errors
/// Returns an object error when the descriptor allocation fails.
pub fn make_slot_descriptor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    location: Option<Fixnum>,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let values = scope.root_many(&[
        Local::from_word(name),
        Local::from_word(location.map_or(Word::NIL, Fixnum::as_word)),
    ]);
    let result = scope.make_simple_vector(runtime, &values)?;
    Ok(scope.get(result).as_word())
}

/// Make an EQL-specializer descriptor understood by this module.
///
/// # Errors
/// Returns an object error when the descriptor allocation fails.
pub fn make_eql_specializer(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let values = scope.root_many(&[Local::from_word(Word::fixnum(1)), Local::from_word(object)]);
    let result = scope.make_simple_vector(runtime, &values)?;
    Ok(scope.get(result).as_word())
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

fn instance_arg(ctx: &ThreadContext, object: Word) -> Result<Instance, ObjectError> {
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
    let mut scope = Scope::new(ctx);
    let mut current = scope.root::<Word>(Local::from_word(class));
    let limit = simple_vector_length(scope.context(), class)?;
    let mut result: HandleVec<'_, Word> = scope.root_many(&[]);
    for _ in 0..=limit {
        let current_word = scope.get(current).as_word();
        result.push(&mut scope, Local::from_word(current_word));
        let parent = class_field(scope.context(), current_word, CLASS_DIRECT_SUPERCLASS)?;
        // A class with more than one direct superclass (multiple
        // inheritance, e.g. a condition class from `ncl-conditions`) stores
        // a list of descriptors here; walk its first (most-specific) parent
        // as a linearization stand-in rather than erroring.
        let parent = if parent.is_cons() {
            car(scope.context(), parent)?
        } else {
            parent
        };
        if parent == Word::NIL {
            break;
        }
        current = scope.root(Local::from_word(parent));
    }
    let result = scope.make_simple_vector(runtime, &result)?;
    Ok(scope.get(result).as_word())
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
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let class = args.required(0)?;
    let field = if simple_vector_length(ctx, class)? > CLASS_EFFECTIVE_SLOTS {
        CLASS_EFFECTIVE_SLOTS
    } else {
        CLASS_SLOTS
    };
    class_field(ctx, class, field)
}

fn class_direct_slots_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    // The descriptor's slot field is the direct slot metadata.  CLASS-SLOTS
    // computes the effective metadata across the superclass chain.
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
    ctx: &ThreadContext,
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use ncl_object::{make_cons, make_simple_vector};

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context");
        (runtime, ctx)
    }

    #[test]
    fn class_precedence_list_reads_cons_parent_and_stops_at_nil() {
        let (runtime, mut ctx) = setup();
        let parent = make_simple_vector(
            &mut ctx,
            &runtime,
            &[Word::fixnum(1), Word::NIL, Word::NIL, Word::fixnum(0)],
        )
        .expect("parent");
        let parent_list = make_cons(&mut ctx, &runtime, parent, Word::NIL).expect("parent list");
        let child = make_simple_vector(
            &mut ctx,
            &runtime,
            &[Word::fixnum(2), parent_list, Word::NIL, Word::fixnum(0)],
        )
        .expect("child");

        let precedence = class_precedence_list(&mut ctx, &runtime, child).expect("precedence");
        assert_eq!(simple_vector_length(&ctx, precedence), Ok(2));
        assert_eq!(simple_vector_ref(&ctx, precedence, 0), Ok(child));
        assert_eq!(simple_vector_ref(&ctx, precedence, 1), Ok(parent));
    }

    #[test]
    fn class_slots_selects_effective_field_only_for_extended_descriptors() {
        let (runtime, mut ctx) = setup();
        let direct = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(10)]).expect("direct");
        let effective =
            make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(20)]).expect("effective");
        let legacy = make_simple_vector(
            &mut ctx,
            &runtime,
            &[Word::fixnum(0), Word::NIL, direct, Word::fixnum(0)],
        )
        .expect("legacy");
        let extended = make_simple_vector(
            &mut ctx,
            &runtime,
            &[
                Word::fixnum(0),
                Word::NIL,
                direct,
                Word::fixnum(0),
                effective,
            ],
        )
        .expect("extended");

        assert_eq!(
            class_slots_builtin_value(&mut ctx, &runtime, legacy),
            direct
        );
        assert_eq!(
            class_slots_builtin_value(&mut ctx, &runtime, extended),
            effective
        );
    }

    fn class_slots_builtin_value(ctx: &mut ThreadContext, runtime: &Runtime, class: Word) -> Word {
        let arguments = [class];
        let args = BuiltinArgs::new(&arguments);
        let mut values = MultipleValues::new();
        class_slots_builtin(ctx, runtime, &args, &mut values).expect("class slots")
    }

    #[test]
    fn location_and_instance_checks_return_type_errors_for_invalid_values() {
        let (runtime, mut ctx) = setup();
        let malformed_slot =
            make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL]).expect("slot");
        assert_eq!(
            location_arg(&ctx, malformed_slot),
            Err(ObjectError::TypeError)
        );
        assert_eq!(instance_arg(&ctx, Word::NIL), Err(ObjectError::TypeError));
    }
}

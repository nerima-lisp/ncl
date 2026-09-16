use crate::object_access::{get, put};
use crate::{
    ObjectError, Runtime, ThreadContext, allocate, instance_offset, make_simple_vector,
    simple_vector_ref, simple_vector_set, widetag,
};
use ncl_sys::Word;

crate::word_newtype!(Instance);

/// Allocate an instance with a simple-vector slot store.
///
/// # Errors
/// Returns an error when either allocation fails.
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_instance(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut class: Word,
    slots: &[Word],
) -> Result<Instance, ObjectError> {
    let mut vector = make_simple_vector(ctx, runtime, slots)?;
    crate::with_root(ctx, &mut class, |ctx, class| {
        crate::with_root(ctx, &mut vector, |ctx, vector| {
            let object = allocate(ctx, runtime, widetag::INSTANCE, 3)?;
            put(ctx, object, instance_offset::CLASS, *class)?;
            put(ctx, object, instance_offset::SLOT_VECTOR, *vector)?;
            put(ctx, object, instance_offset::GENERATION, Word::fixnum(0))?;
            Ok(object.into())
        })
    })
}

/// Read an instance class.
///
/// # Errors
/// Returns an error when the object is not an instance.
pub fn instance_class(ctx: &ThreadContext, object: Instance) -> Result<Word, ObjectError> {
    get(
        ctx,
        object.into(),
        widetag::INSTANCE,
        instance_offset::CLASS,
    )
}

/// Read an instance slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn slot_ref(ctx: &ThreadContext, object: Instance, index: usize) -> Result<Word, ObjectError> {
    let vector = get(
        ctx,
        object.into(),
        widetag::INSTANCE,
        instance_offset::SLOT_VECTOR,
    )?;
    simple_vector_ref(ctx, vector, index)
}

/// Write an instance slot.
///
/// # Errors
/// Returns an error when the object or slot is invalid.
pub fn slot_set(
    ctx: &mut ThreadContext,
    object: Instance,
    index: usize,
    value: Word,
) -> Result<(), ObjectError> {
    let vector = get(
        ctx,
        object.into(),
        widetag::INSTANCE,
        instance_offset::SLOT_VECTOR,
    )?;
    simple_vector_set(ctx, vector, index, value)
}
